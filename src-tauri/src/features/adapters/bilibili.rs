// 哔哩哔哩适配器（L2 谨慎级）
// 方式：网页版扫码登录 → SESSDATA 会话仅存本机 → 官方网页接口读取关注列表 → 接口取关
// 合规：用户仅操作自己账号；会话不离开本机；取关限速 + 风控熔断
use crate::db::DbState;
use crate::error::{AppError, AppResult};
use crate::models::NormalizedAccount;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

const PLATFORM: &str = "bilibili";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Credentials {
    sessdata: String,
    bili_jct: String,
    dede_user_id: String,
    uname: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrStart {
    pub qrcode_key: String,
    pub url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrPoll {
    /// waiting（未扫码）| scanned（已扫码待确认）| expired（过期）| success（登录成功）
    pub status: String,
    pub uname: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub logged_in: bool,
    pub uname: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    pub total: usize,
    pub accounts: Vec<NormalizedAccount>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UnfollowProgress {
    pub total: usize,
    pub done: usize,
    pub mid: String,
    pub display_name: String,
    pub success: bool,
    pub message: String,
    pub finished: bool,
    pub stopped: bool,
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("build http client")
}

fn creds_cookie(c: &Credentials) -> String {
    format!(
        "SESSDATA={}; bili_jct={}; DedeUserID={}",
        c.sessdata, c.bili_jct, c.dede_user_id
    )
}

// ---------- 凭证存取 ----------

fn load_creds(conn: &rusqlite::Connection) -> AppResult<Option<Credentials>> {
    let row: Option<String> = conn
        .query_row(
            "SELECT cookie_json FROM credentials WHERE platform = ?1",
            rusqlite::params![PLATFORM],
            |r| r.get(0),
        )
        .ok();
    match row {
        Some(json) => Ok(Some(serde_json::from_str(&json)?)),
        None => Ok(None),
    }
}

fn save_creds(conn: &rusqlite::Connection, c: &Credentials) -> AppResult<()> {
    let ts = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO credentials (platform, cookie_json, account_name, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(platform) DO UPDATE SET
            cookie_json = excluded.cookie_json,
            account_name = excluded.account_name,
            updated_at = excluded.updated_at",
        rusqlite::params![
            PLATFORM,
            serde_json::to_string(c)?,
            c.uname,
            ts
        ],
    )?;
    Ok(())
}

/// 调 /nav 校验会话并补全 uname
async fn verify_login(client: &reqwest::Client, c: &Credentials) -> AppResult<(bool, String, Option<i64>)> {
    let v: Value = client
        .get("https://api.bilibili.com/x/web-interface/nav")
        .header("Cookie", creds_cookie(c))
        .send()
        .await?
        .json()
        .await?;
    let is_login = v["data"]["isLogin"].as_bool().unwrap_or(false);
    let uname = v["data"]["uname"].as_str().unwrap_or("").to_string();
    let mid = v["data"]["mid"].as_i64();
    Ok((is_login, uname, mid))
}

// ---------- 命令：扫码登录 ----------

#[tauri::command]
pub async fn bilibili_qr_generate() -> AppResult<QrStart> {
    let client = http();
    let v: Value = client
        .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
        .send()
        .await?
        .json()
        .await?;
    if v["code"].as_i64() != Some(0) {
        return Err(AppError::Validation(format!(
            "二维码生成失败: {}",
            v["message"].as_str().unwrap_or("未知错误")
        )));
    }
    Ok(QrStart {
        qrcode_key: v["data"]["qrcode_key"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        url: v["data"]["url"].as_str().unwrap_or_default().to_string(),
    })
}

#[tauri::command]
pub async fn bilibili_qr_poll(
    state: State<'_, DbState>,
    qrcode_key: String,
) -> AppResult<QrPoll> {
    let client = http();
    let v: Value = client
        .get("https://passport.bilibili.com/x/passport-login/web/qrcode/poll")
        .query(&[("qrcode_key", qrcode_key.as_str())])
        .send()
        .await?
        .json()
        .await?;
    let code = v["data"]["code"].as_i64().unwrap_or(-1);
    match code {
        86101 => Ok(QrPoll {
            status: "waiting".into(),
            uname: None,
        }),
        86090 => Ok(QrPoll {
            status: "scanned".into(),
            uname: None,
        }),
        86038 | 86017 => Ok(QrPoll {
            status: "expired".into(),
            uname: None,
        }),
        0 => {
            // 成功：回调 url 的 query 中携带 SESSDATA / bili_jct / DedeUserID
            let url = v["data"]["url"].as_str().unwrap_or_default();
            let parsed = url::Url::parse(url)
                .map_err(|_| AppError::Validation("登录回包解析失败".into()))?;
            let q = parsed.query_pairs();
            let get = |k: &str| {
                q.clone()
                    .find(|(key, _)| key == k)
                    .map(|(_, val)| val.to_string())
                    .unwrap_or_default()
            };
            let sessdata = get("SESSDATA");
            let bili_jct = get("bili_jct");
            let dede_user_id = get("DedeUserID");
            if sessdata.is_empty() || bili_jct.is_empty() {
                return Err(AppError::Validation(
                    "登录成功但未获取到会话凭证，请重试".into(),
                ));
            }
            let mut creds = Credentials {
                sessdata,
                bili_jct,
                dede_user_id,
                uname: None,
            };
            let (ok, uname, mid) = verify_login(&client, &creds).await?;
            if !ok {
                return Err(AppError::Validation("会话校验失败，请重试".into()));
            }
            if creds.dede_user_id.is_empty() {
                if let Some(m) = mid {
                    creds.dede_user_id = m.to_string();
                }
            }
            creds.uname = Some(uname.clone());
            {
                let conn = state.0.lock().unwrap();
                save_creds(&conn, &creds)?;
            }
            log::info!("bilibili login success: {uname}");
            Ok(QrPoll {
                status: "success".into(),
                uname: Some(uname),
            })
        }
        _ => Err(AppError::Validation(format!(
            "扫码状态异常（code={code}）"
        ))),
    }
}

#[tauri::command]
pub async fn bilibili_status(state: State<'_, DbState>) -> AppResult<ConnectionStatus> {
    let client = http();
    let creds = {
        let conn = state.0.lock().unwrap();
        load_creds(&conn)?
    };
    if let Some(c) = creds {
        let (ok, uname, _) = verify_login(&client, &c).await?;
        if ok {
            return Ok(ConnectionStatus {
                logged_in: true,
                uname: Some(uname),
            });
        }
    }
    Ok(ConnectionStatus {
        logged_in: false,
        uname: None,
    })
}

#[tauri::command]
pub async fn bilibili_logout(state: State<'_, DbState>) -> AppResult<()> {
    let conn = state.0.lock().unwrap();
    conn.execute(
        "DELETE FROM credentials WHERE platform = ?1",
        rusqlite::params![PLATFORM],
    )?;
    Ok(())
}

// ---------- 命令：拉取关注列表 ----------

#[tauri::command]
pub async fn bilibili_fetch_follows(state: State<'_, DbState>) -> AppResult<FetchResult> {
    let client = http();
    let creds = {
        let conn = state.0.lock().unwrap();
        load_creds(&conn)?
    }
    .ok_or_else(|| AppError::Validation("尚未登录哔哩哔哩账号".into()))?;

    let cookie = creds_cookie(&creds);
    let vmid = creds.dede_user_id.clone();
    if vmid.is_empty() {
        return Err(AppError::Validation("会话缺少账号 ID，请重新登录".into()));
    }

    let mut accounts: Vec<NormalizedAccount> = Vec::new();
    let mut total: usize = 0;
    let ps: i64 = 50;
    let mut pn: i64 = 1;
    loop {
        let v: Value = client
            .get("https://api.bilibili.com/x/relation/followings")
            .header("Cookie", &cookie)
            .query(&[
                ("vmid", vmid.as_str()),
                ("pn", pn.to_string().as_str()),
                ("ps", ps.to_string().as_str()),
                ("order", "desc"),
            ])
            .send()
            .await?
            .json()
            .await?;
        if v["code"].as_i64() != Some(0) {
            return Err(AppError::Validation(format!(
                "拉取关注列表失败: {}（code={}）",
                v["message"].as_str().unwrap_or("未知"),
                v["code"]
            )));
        }
        let data = &v["data"];
        if pn == 1 {
            total = data["total"].as_u64().unwrap_or(0) as usize;
        }
        let list = data["list"].as_array();
        let Some(list) = list else { break };
        if list.is_empty() {
            break;
        }
        for item in list {
            let mid = item["mid"].as_i64().unwrap_or_default().to_string();
            let uname = item["uname"].as_str().unwrap_or("未知").to_string();
            let mtime = item["mtime"].as_i64();
            let sign = item["sign"].as_str().unwrap_or("");
            // 官方认证 type==1 为机构，0 为个人认证；无认证按个人号
            let account_type = match item["official_verify"]["type"].as_i64() {
                Some(1) => "brand",
                Some(0) => "official",
                _ => "personal",
            };
            accounts.push(NormalizedAccount {
                platform: PLATFORM.into(),
                platform_account_id: mid.clone(),
                display_name: uname,
                account_type: account_type.into(),
                homepage_url: Some(format!("https://space.bilibili.com/{mid}")),
                followed_at: mtime,
                interaction_level: "unknown".into(),
                note: (!sign.is_empty()).then(|| sign.to_string()),
            });
        }
        if accounts.len() >= total || (list.len() as i64) < ps {
            break;
        }
        pn += 1;
        if pn > 200 {
            break; // 防御性上限（1 万个关注）
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    log::info!("bilibili fetched {}/{} follows", accounts.len(), total);
    Ok(FetchResult { total, accounts })
}

// ---------- 命令：批量取关（限速 + 风控熔断 + 事件推送） ----------

#[tauri::command]
pub async fn bilibili_unfollow_batch(
    app: AppHandle,
    state: State<'_, DbState>,
    mids: Vec<String>,
) -> AppResult<()> {
    let client = http();
    let creds = {
        let conn = state.0.lock().unwrap();
        load_creds(&conn)?
    }
    .ok_or_else(|| AppError::Validation("尚未登录哔哩哔哩账号".into()))?;

    let total = mids.len();
    let emit = |p: UnfollowProgress| {
        let _ = app.emit("unfollow-progress", p);
    };

    // mid -> 昵称（用于进度展示）
    let name_map: std::collections::HashMap<String, String> = {
        let conn = state.0.lock().unwrap();
        let mut m = std::collections::HashMap::new();
        for mid in &mids {
            if let Ok(name) = conn.query_row(
                "SELECT display_name FROM accounts WHERE platform = 'bilibili' AND platform_account_id = ?1",
                rusqlite::params![mid],
                |r| r.get::<_, String>(0),
            ) {
                m.insert(mid.clone(), name);
            }
        }
        m
    };

    for (i, mid) in mids.iter().enumerate() {
        let display_name = name_map.get(mid).cloned().unwrap_or_else(|| mid.clone());

        // 限速：3~8 秒随机（首个操作前也等待，给用户反悔窗口）
        let delay = rand::thread_rng().gen_range(3000..8000);
        tokio::time::sleep(Duration::from_millis(delay)).await;

        let form = [
            ("fid", mid.as_str()),
            ("act", "2"), // 2 = 取关
            ("re_src", "1"),
            ("csrf", creds.bili_jct.as_str()),
        ];
        let result = client
            .post("https://api.bilibili.com/x/relation/modify")
            .header("Cookie", creds_cookie(&creds))
            .header("Referer", "https://space.bilibili.com/")
            .form(&form)
            .send()
            .await;

        let (success, message, stopped) = match result {
            Ok(resp) => {
                let status = resp.status();
                let body: Value = match resp.json().await {
                    Ok(b) => b,
                    Err(_) => Value::Null,
                };
                let code = body["code"].as_i64();
                match code {
                    Some(0) => {
                        // 更新本地状态为已取关
                        let conn = state.0.lock().unwrap();
                        let _ = conn.execute(
                            "UPDATE accounts SET status = 'unfollowed', updated_at = ?2
                             WHERE platform = 'bilibili' AND platform_account_id = ?1",
                            rusqlite::params![mid, chrono::Utc::now().timestamp()],
                        );
                        (true, "已取关".to_string(), false)
                    }
                    // 风控/校验类错误：熔断，停止整批
                    Some(-352) | Some(-799) | Some(87000) => (
                        false,
                        format!("触发平台风控校验（code={code:?}），已停止，剩余账号未处理"),
                        true,
                    ),
                    Some(-111) | Some(-101) => (
                        false,
                        "登录已失效，请重新扫码登录后再试".to_string(),
                        true,
                    ),
                    _ => (
                        false,
                        format!(
                            "取关失败（http {}，code={:?}）{}",
                            status,
                            code,
                            body["message"].as_str().map(|m| format!("：{m}")).unwrap_or_default()
                        ),
                        false,
                    ),
                }
            }
            Err(e) => (false, format!("网络错误: {e}"), false),
        };

        let done = i + 1;
        emit(UnfollowProgress {
            total,
            done,
            mid: mid.clone(),
            display_name: display_name.clone(),
            success,
            message,
            finished: done >= total,
            stopped,
        });

        if stopped {
            log::warn!("unfollow batch stopped at {done}/{total} due to risk control");
            break;
        }
    }
    Ok(())
}
