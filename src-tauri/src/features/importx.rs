use crate::db::DbState;
use crate::error::{AppError, AppResult};
use crate::features::accounts::map_account;
use crate::models::*;
use chrono::TimeZone;
use rusqlite::params_from_iter;
use serde_json::Value;
use std::collections::HashMap;
use tauri::State;
use uuid::Uuid;

const CSV_HEADERS: &[&str] = &[
    "platform",
    "platform_account_id",
    "display_name",
    "account_type",
    "homepage_url",
    "followed_at",
    "interaction_level",
    "note",
];

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// 规范化过程中的两种非正常情况
enum RowProblem {
    /// 模板示例行等，静默跳过
    Skip,
    /// 数据错误，计入错误报告
    Invalid(String),
}

fn parse_followed_at(raw: &str) -> Result<Option<i64>, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Ok(None);
    }
    // 纯数字：unix 秒
    if s.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(v) = s.parse::<i64>() {
            if v >= 1_000_000_000 {
                return Ok(Some(v));
            }
        }
        return Err(format!("关注时间格式无法识别: {s}（支持 YYYY-MM-DD 或 unix 秒）"));
    }
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d", "%Y/%m/%d %H:%M:%S", "%Y/%m/%d"] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(Some(dt.and_utc().timestamp()));
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, fmt) {
            return Ok(Some(d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp()));
        }
    }
    Err(format!("关注时间格式无法识别: {s}（支持 YYYY-MM-DD 或 unix 秒）"))
}

fn normalize(row: ImportRow) -> Result<NormalizedAccount, RowProblem> {
    let platform = row.platform.unwrap_or_default().trim().to_lowercase();
    if platform.is_empty() {
        return Err(RowProblem::Invalid("缺少 platform 字段".into()));
    }
    let pid = row.platform_account_id.unwrap_or_default().trim().to_string();
    if pid.is_empty() {
        return Err(RowProblem::Invalid("缺少 platformAccountId 字段".into()));
    }
    // 模板示例行
    if platform == "example" || pid.eq_ignore_ascii_case("example") {
        return Err(RowProblem::Skip);
    }

    let followed_at = match row.followed_at {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => parse_followed_at(&s).map_err(RowProblem::Invalid)?,
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        _ => return Err(RowProblem::Invalid("followedAt 必须是日期字符串或 unix 秒".into())),
    };

    let account_type = row
        .account_type
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let account_type = if ACCOUNT_TYPES.contains(&account_type.as_str()) {
        account_type
    } else {
        "unknown".to_string()
    };

    let interaction = row
        .interaction_level
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let interaction = if INTERACTION_LEVELS.contains(&interaction.as_str()) {
        interaction
    } else {
        "unknown".to_string()
    };

    let display_name = row
        .display_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| pid.clone());
    let homepage_url = row
        .homepage_url
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let note = row
        .note
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(NormalizedAccount {
        platform,
        platform_account_id: pid,
        display_name,
        account_type,
        homepage_url,
        followed_at,
        interaction_level: interaction,
        note,
    })
}

fn empty_parse_result() -> ParseResult {
    ParseResult {
        total_rows: 0,
        valid_count: 0,
        skipped_count: 0,
        errors: Vec::new(),
        accounts: Vec::new(),
    }
}

fn parse_csv(content: &str) -> AppResult<ParseResult> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(content.as_bytes());

    // clone：headers() 返回借用 reader 的引用，后续 records() 需可变借用
    let headers = rdr.headers()?.clone();
    let mut idx: HashMap<String, usize> = HashMap::new();
    for (i, h) in headers.iter().enumerate() {
        let key = h.trim().to_lowercase().replace(['_', '-', ' '], "");
        idx.insert(key, i);
    }
    let get = |rec: &csv::StringRecord, keys: &[&str]| -> Option<String> {
        for k in keys {
            if let Some(i) = idx.get(*k) {
                if let Some(v) = rec.get(*i) {
                    let v = v.trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        None
    };

    let mut result = empty_parse_result();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec?;
        // 整行空白跳过
        if rec.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        result.total_rows += 1;
        let row_no = i + 2; // 含表头
        let row = ImportRow {
            platform: get(&rec, &["platform"]),
            platform_account_id: get(&rec, &["platformaccountid", "platformid", "id"]),
            display_name: get(&rec, &["displayname", "name", "nickname"]),
            account_type: get(&rec, &["accounttype", "type"]),
            homepage_url: get(&rec, &["homepageurl", "url", "link"]),
            followed_at: get(&rec, &["followedat", "followtime"]).map(Value::String),
            interaction_level: get(&rec, &["interactionlevel", "interaction"]),
            note: get(&rec, &["note", "remark"]),
        };
        match normalize(row) {
            Ok(acc) => {
                result.valid_count += 1;
                result.accounts.push(acc);
            }
            Err(RowProblem::Skip) => result.skipped_count += 1,
            Err(RowProblem::Invalid(msg)) => result.errors.push(RowError {
                row: row_no,
                message: msg,
            }),
        }
    }
    Ok(result)
}

fn parse_json(content: &str) -> AppResult<ParseResult> {
    let rows: Vec<ImportRow> = match serde_json::from_str(content) {
        Ok(r) => r,
        Err(e) => {
            return Ok(ParseResult {
                errors: vec![RowError {
                    row: 0,
                    message: format!("JSON 文件解析失败: {e}"),
                }],
                ..empty_parse_result()
            })
        }
    };
    let mut result = empty_parse_result();
    for (i, row) in rows.into_iter().enumerate() {
        // 完全空对象跳过
        let is_empty = row.platform.is_none() && row.platform_account_id.is_none();
        if is_empty {
            continue;
        }
        result.total_rows += 1;
        match normalize(row) {
            Ok(acc) => {
                result.valid_count += 1;
                result.accounts.push(acc);
            }
            Err(RowProblem::Skip) => result.skipped_count += 1,
            Err(RowProblem::Invalid(msg)) => result.errors.push(RowError {
                row: i + 1,
                message: msg,
            }),
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn parse_import_file(path: String) -> AppResult<ParseResult> {
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let bytes = std::fs::read(&path)?;
    let content = String::from_utf8_lossy(&bytes);
    match ext.as_str() {
        "csv" => parse_csv(&content),
        "json" => parse_json(&content),
        other => Err(AppError::UnsupportedFormat(other.to_string())),
    }
}

/// 预览确认后写入：按 (platform, platform_account_id) 去重 upsert
#[tauri::command]
pub async fn commit_import(
    state: State<'_, DbState>,
    accounts: Vec<NormalizedAccount>,
) -> AppResult<ImportResult> {
    let mut conn = state.0.lock().unwrap();
    let tx = conn.transaction()?;
    let mut inserted = 0usize;
    let mut updated = 0usize;
    let ts = now();

    for a in &accounts {
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM accounts WHERE platform = ?1 AND platform_account_id = ?2",
                rusqlite::params![a.platform, a.platform_account_id],
                |r| r.get(0),
            )
            .ok();

        match existing {
            Some(id) => {
                tx.execute(
                    "UPDATE accounts SET
                        display_name = ?1, account_type = ?2, homepage_url = ?3,
                        followed_at = COALESCE(?4, followed_at),
                        interaction_level = ?5, note = ?6, updated_at = ?7
                     WHERE id = ?8",
                    rusqlite::params![
                        a.display_name,
                        a.account_type,
                        a.homepage_url,
                        a.followed_at,
                        a.interaction_level,
                        a.note,
                        ts,
                        id
                    ],
                )?;
                updated += 1;
            }
            None => {
                tx.execute(
                    "INSERT INTO accounts
                        (id, platform, platform_account_id, display_name, avatar_path,
                         account_type, homepage_url, followed_at, interaction_level,
                         interaction_metrics, status, note, imported_at, updated_at)
                     VALUES (?1,?2,?3,?4,NULL,?5,?6,?7,?8,NULL,'following',?9,?10,?10)",
                    rusqlite::params![
                        Uuid::new_v4().to_string(),
                        a.platform,
                        a.platform_account_id,
                        a.display_name,
                        a.account_type,
                        a.homepage_url,
                        a.followed_at,
                        a.interaction_level,
                        a.note,
                        ts
                    ],
                )?;
                inserted += 1;
            }
        }
    }
    tx.commit()?;
    log::info!("import committed: {inserted} inserted, {updated} updated");
    Ok(ImportResult { inserted, updated })
}

/// 写入导入模板（CSV 或 JSON）
#[tauri::command]
pub async fn write_template(path: String, format: String) -> AppResult<()> {
    let format = format.to_lowercase();
    match format.as_str() {
        "csv" => {
            let mut wtr = csv::Writer::from_path(&path)?;
            wtr.write_record(CSV_HEADERS)?;
            wtr.write_record([
                "example",
                "demo001",
                "示例账号（导入前请删除本行）",
                "personal",
                "https://example.com/demo001",
                "2024-01-15",
                "low",
                "模板示例行，可删除",
            ])?;
            wtr.flush()?;
        }
        "json" => {
            let example = serde_json::json!([
                {
                    "platform": "example",
                    "platformAccountId": "demo001",
                    "displayName": "示例账号（导入前请删除本行）",
                    "accountType": "personal",
                    "homepageUrl": "https://example.com/demo001",
                    "followedAt": "2024-01-15",
                    "interactionLevel": "low",
                    "note": "模板示例行，可删除"
                }
            ]);
            std::fs::write(&path, serde_json::to_string_pretty(&example)?)?;
        }
        other => return Err(AppError::UnsupportedFormat(other.to_string())),
    }
    Ok(())
}

/// 导出账号为 CSV/JSON。account_ids 为 None 时导出全部
#[tauri::command]
pub async fn export_accounts(
    state: State<'_, DbState>,
    path: String,
    format: String,
    account_ids: Option<Vec<String>>,
) -> AppResult<usize> {
    let conn = state.0.lock().unwrap();

    let accounts: Vec<Account> = if let Some(ids) = account_ids {
        if ids.is_empty() {
            return Ok(0);
        }
        let placeholders = (0..ids.len()).map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT * FROM accounts WHERE id IN ({placeholders})");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(ids.iter()), map_account)?;
        let mut v = Vec::new();
        for r in rows {
            v.push(r?);
        }
        v
    } else {
        let mut stmt = conn.prepare("SELECT * FROM accounts ORDER BY platform, updated_at DESC")?;
        let rows = stmt.query_map([], map_account)?;
        let mut v = Vec::new();
        for r in rows {
            v.push(r?);
        }
        v
    };

    let format = format.to_lowercase();
    let count = accounts.len();
    match format.as_str() {
        "csv" => {
            let mut wtr = csv::Writer::from_path(&path)?;
            wtr.write_record(CSV_HEADERS)?;
            for a in &accounts {
                let followed = a
                    .followed_at
                    .and_then(|t| chrono::Utc.timestamp_opt(t, 0).single())
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default();
                wtr.write_record([
                    &a.platform,
                    &a.platform_account_id,
                    &a.display_name,
                    &a.account_type,
                    a.homepage_url.as_deref().unwrap_or(""),
                    &followed,
                    &a.interaction_level,
                    a.note.as_deref().unwrap_or(""),
                ])?;
            }
            wtr.flush()?;
        }
        "json" => {
            let arr: Vec<Value> = accounts
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "platform": a.platform,
                        "platformAccountId": a.platform_account_id,
                        "displayName": a.display_name,
                        "accountType": a.account_type,
                        "homepageUrl": a.homepage_url,
                        "followedAt": a.followed_at,
                        "interactionLevel": a.interaction_level,
                        "note": a.note,
                    })
                })
                .collect();
            std::fs::write(&path, serde_json::to_string_pretty(&arr)?)?;
        }
        other => return Err(AppError::UnsupportedFormat(other.to_string())),
    }
    Ok(count)
}
