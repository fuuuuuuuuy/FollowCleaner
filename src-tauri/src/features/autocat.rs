// 自动分类：规则引擎一次性执行（V0.6）
// 规则：按账号认证类型 / 按关注时长 / 自定义关键词（匹配昵称与简介）
// 产物：统一挂在根级父分类「自动分类」下；重跑幂等（INSERT OR IGNORE）
// 预留：规则持久化、同步后自动重跑、按平台限定范围 —— 后续版本
use crate::db::DbState;
use crate::error::{AppError, AppResult};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tauri::State;
use uuid::Uuid;

const PARENT_NAME: &str = "自动分类";
const DAY: i64 = 86_400;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KeywordRule {
    pub category_name: String,
    /// 多个关键词任一命中即归入该分类
    pub keywords: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoCategorizeArgs {
    /// verifyType | followAge | keywords
    pub rule_type: String,
    pub keyword_rules: Option<Vec<KeywordRule>>,
    pub dry_run: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AutoCategorizeGroup {
    pub category_name: String,
    pub count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoCategorizeResult {
    pub groups: Vec<AutoCategorizeGroup>,
    pub total_assigned: usize,
    pub dry_run: bool,
}

struct AccountRow {
    id: String,
    display_name: String,
    account_type: String,
    followed_at: Option<i64>,
    note: Option<String>,
}

fn type_label(t: &str) -> &str {
    match t {
        "brand" => "机构官方",
        "official" => "个人认证",
        "subscription_account" => "公众号",
        "video_account" => "视频号",
        "personal" => "普通账号",
        _ => "其他账号",
    }
}

fn follow_age_label(days: i64) -> &'static str {
    if days < 30 {
        "1个月内关注"
    } else if days < 90 {
        "1~3个月前关注"
    } else if days < 365 {
        "3~12个月前关注"
    } else {
        "1年前关注"
    }
}

/// 读取当前在关注中的账号（已取关的不参与分类）
fn load_accounts(conn: &Connection) -> AppResult<Vec<AccountRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, account_type, followed_at, note
         FROM accounts WHERE status = 'following'",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(AccountRow {
                id: r.get(0)?,
                display_name: r.get(1)?,
                account_type: r.get(2)?,
                followed_at: r.get(3)?,
                note: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn build_groups(rule: &AutoCategorizeArgs, accounts: &[AccountRow]) -> AppResult<BTreeMap<String, Vec<String>>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    match rule.rule_type.as_str() {
        "verifyType" => {
            for a in accounts {
                groups
                    .entry(type_label(&a.account_type).to_string())
                    .or_default()
                    .push(a.id.clone());
            }
        }
        "followAge" => {
            let now = Utc::now().timestamp();
            for a in accounts {
                let label = match a.followed_at {
                    Some(ts) if ts > 0 => follow_age_label((now - ts) / DAY),
                    _ => "未知时间",
                };
                groups.entry(label.to_string()).or_default().push(a.id.clone());
            }
        }
        "keywords" => {
            let rules = rule
                .keyword_rules
                .as_ref()
                .filter(|rs| !rs.is_empty())
                .ok_or_else(|| AppError::Validation("请至少填写一条关键词规则".into()))?;
            // 规则校验：分类名非空、关键词非空
            for r in rules {
                if r.category_name.trim().is_empty() {
                    return Err(AppError::Validation("关键词规则的分类名不能为空".into()));
                }
                if r.keywords.iter().all(|k| k.trim().is_empty()) {
                    return Err(AppError::Validation(format!(
                        "分类「{}」至少需要一个关键词",
                        r.category_name
                    )));
                }
            }
            for r in rules {
                let kws: Vec<String> = r
                    .keywords
                    .iter()
                    .map(|k| k.trim().to_lowercase())
                    .filter(|k| !k.is_empty())
                    .collect();
                for a in accounts {
                    let name = a.display_name.to_lowercase();
                    let note = a.note.as_deref().unwrap_or("").to_lowercase();
                    if kws.iter().any(|k| name.contains(k.as_str()) || note.contains(k.as_str())) {
                        groups
                            .entry(r.category_name.trim().to_string())
                            .or_default()
                            .push(a.id.clone());
                    }
                }
            }
        }
        other => {
            return Err(AppError::Validation(format!("未知规则类型: {other}")));
        }
    }
    groups.retain(|_, ids| !ids.is_empty());
    Ok(groups)
}

/// 按名称找分类（同父下），没有则创建；返回分类 id
fn ensure_category(conn: &Connection, name: &str, parent_id: Option<&str>) -> AppResult<String> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM categories WHERE name = ?1 AND parent_id IS ?2 LIMIT 1",
            params![name, parent_id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = Uuid::new_v4().to_string();
    let ts = Utc::now().timestamp();
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM categories WHERE parent_id IS ?1",
        params![parent_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO categories (id, name, parent_id, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, name, parent_id, next_order, ts],
    )?;
    Ok(id)
}

#[tauri::command]
pub async fn auto_categorize(
    state: State<'_, DbState>,
    args: AutoCategorizeArgs,
) -> AppResult<AutoCategorizeResult> {
    let mut conn = state.0.lock().unwrap();
    let accounts = load_accounts(&conn)?;
    if accounts.is_empty() {
        return Err(AppError::Validation("当前没有可分类的在关注账号，请先导入或同步".into()));
    }
    let groups = build_groups(&args, &accounts)?;
    if groups.is_empty() {
        return Err(AppError::Validation(
            "没有账号命中规则，请调整规则后重试（关键词规则区分大小写不敏感、匹配昵称与简介）".into(),
        ));
    }

    if args.dry_run {
        return Ok(AutoCategorizeResult {
            groups: groups
                .iter()
                .map(|(name, ids)| AutoCategorizeGroup {
                    category_name: name.clone(),
                    count: ids.len(),
                })
                .collect(),
            total_assigned: 0,
            dry_run: true,
        });
    }

    let tx = conn.transaction()?;
    let parent_id = ensure_category(&tx, PARENT_NAME, None)?;
    let mut total_assigned = 0usize;
    let mut result_groups = Vec::new();
    for (name, ids) in &groups {
        let cid = ensure_category(&tx, name, Some(&parent_id))?;
        for aid in ids {
            let n = tx.execute(
                "INSERT OR IGNORE INTO account_categories (account_id, category_id)
                 VALUES (?1, ?2)",
                params![aid, cid],
            )?;
            total_assigned += n;
        }
        result_groups.push(AutoCategorizeGroup {
            category_name: name.clone(),
            count: ids.len(),
        });
    }
    tx.commit()?;

    log::info!(
        "auto_categorize rule={} -> {} groups, {} new assignments",
        args.rule_type,
        result_groups.len(),
        total_assigned
    );
    Ok(AutoCategorizeResult {
        groups: result_groups,
        total_assigned,
        dry_run: false,
    })
}
