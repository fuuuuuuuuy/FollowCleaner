use crate::db::DbState;
use crate::error::{AppError, AppResult};
use crate::models::{Account, AccountFilter, AccountWithCategories, NameCount, OverviewCounts};
use rusqlite::params_from_iter;
use std::collections::HashMap;
use tauri::State;

pub(crate) fn map_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: row.get("id")?,
        platform: row.get("platform")?,
        platform_account_id: row.get("platform_account_id")?,
        display_name: row.get("display_name")?,
        avatar_path: row.get("avatar_path")?,
        account_type: row.get("account_type")?,
        homepage_url: row.get("homepage_url")?,
        followed_at: row.get("followed_at")?,
        interaction_level: row.get("interaction_level")?,
        interaction_metrics: row.get("interaction_metrics")?,
        status: row.get("status")?,
        note: row.get("note")?,
        imported_at: row.get("imported_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// 根据过滤条件动态构造 WHERE 与参数
fn build_where(filter: &AccountFilter) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
    let mut sql = String::from("WHERE 1=1");
    let mut p: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(q) = &filter.search {
        let q = q.trim();
        if !q.is_empty() {
            let like = format!("%{q}%");
            sql.push_str(
                " AND (display_name LIKE ?1 OR note LIKE ?1 OR platform_account_id LIKE ?1)",
            );
            // ?1 复用同一值，后续 `?` 自动从 ?2 开始编号
            p.push(Box::new(like));
        }
    }
    if !filter.platforms.is_empty() {
        let placeholders = (0..filter.platforms.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(" AND platform IN ({placeholders})"));
        for v in &filter.platforms {
            p.push(Box::new(v.clone()));
        }
    }
    if !filter.account_types.is_empty() {
        let placeholders = (0..filter.account_types.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(" AND account_type IN ({placeholders})"));
        for v in &filter.account_types {
            p.push(Box::new(v.clone()));
        }
    }
    if !filter.interaction_levels.is_empty() {
        let placeholders = (0..filter.interaction_levels.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(" AND interaction_level IN ({placeholders})"));
        for v in &filter.interaction_levels {
            p.push(Box::new(v.clone()));
        }
    }
    if !filter.statuses.is_empty() {
        let placeholders = (0..filter.statuses.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(" AND status IN ({placeholders})"));
        for v in &filter.statuses {
            p.push(Box::new(v.clone()));
        }
    }
    if let Some(t) = filter.followed_after {
        sql.push_str(" AND followed_at IS NOT NULL AND followed_at >= ?");
        p.push(Box::new(t));
    }
    if let Some(t) = filter.followed_before {
        sql.push_str(" AND followed_at IS NOT NULL AND followed_at <= ?");
        p.push(Box::new(t));
    }
    // 关注时长档位：多选时各档 OR 组合，再与其他条件 AND
    if !filter.follow_age_buckets.is_empty() {
        let now = chrono::Utc::now().timestamp();
        let day = 86_400i64;
        // 档位 -> [下界(含), 上界(不含))；None 表示该侧不设限
        let ranges: Vec<(Option<i64>, Option<i64>)> = filter
            .follow_age_buckets
            .iter()
            .filter_map(|b| match b.as_str() {
                "6m" => Some((Some(now - 180 * day), Some(now + 1))),
                "6-12m" => Some((Some(now - 365 * day), Some(now - 180 * day))),
                "1-3y" => Some((Some(now - 3 * 365 * day), Some(now - 365 * day))),
                "3y+" => Some((None, Some(now - 3 * 365 * day))),
                _ => None,
            })
            .collect();
        if !ranges.is_empty() {
            let mut conds = Vec::new();
            for (lo, hi) in &ranges {
                let mut c = String::new();
                if let Some(l) = lo {
                    c.push_str("followed_at >= ?");
                    p.push(Box::new(*l));
                }
                if let Some(h) = hi {
                    if !c.is_empty() {
                        c.push_str(" AND ");
                    }
                    c.push_str("followed_at < ?");
                    p.push(Box::new(*h));
                }
                if c.is_empty() {
                    c.push_str("1=1");
                }
                conds.push(c);
            }
            sql.push_str(&format!(
                " AND followed_at IS NOT NULL AND ({})",
                conds.join(" OR ")
            ));
        }
    }
    if let Some(cid) = &filter.category_id {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM account_categories ac WHERE ac.account_id = accounts.id AND ac.category_id = ?)",
        );
        p.push(Box::new(cid.clone()));
    }
    if filter.uncategorized == Some(true) {
        sql.push_str(
            " AND NOT EXISTS (SELECT 1 FROM account_categories ac WHERE ac.account_id = accounts.id)",
        );
    }

    // 排序（白名单，防注入）
    let order = if filter.order.as_deref() == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let order_sql = match filter.sort.as_deref() {
        Some("name") => format!("ORDER BY display_name {order}"),
        Some("interaction") => format!(
            "ORDER BY CASE interaction_level
                WHEN 'high' THEN 4 WHEN 'medium' THEN 3 WHEN 'low' THEN 2 WHEN 'none' THEN 1
                ELSE 0 END {order}"
        ),
        Some("imported") => format!("ORDER BY imported_at {order}"),
        _ => format!("ORDER BY followed_at IS NULL, followed_at {order}"),
    };
    sql.push(' ');
    sql.push_str(&order_sql);

    (sql, p)
}

#[tauri::command]
pub async fn query_accounts(
    state: State<'_, DbState>,
    filter: AccountFilter,
) -> AppResult<Vec<AccountWithCategories>> {
    let conn = state.0.lock().unwrap();
    let (where_sql, params) = build_where(&filter);
    let sql = format!("SELECT * FROM accounts {where_sql} LIMIT 200000");

    let mut accounts: Vec<Account> = {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(params.iter()), map_account)?;
        let mut v = Vec::new();
        for r in rows {
            v.push(r?);
        }
        v
    };

    // 一次性拉取全部归属关系（避免 N+1）
    let mut cat_map: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt =
            conn.prepare("SELECT account_id, category_id FROM account_categories")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for r in rows {
            let (aid, cid) = r?;
            cat_map.entry(aid).or_default().push(cid);
        }
    }

    let result = accounts
        .drain(..)
        .map(|account| {
            let category_ids = cat_map.remove(&account.id).unwrap_or_default();
            AccountWithCategories {
                account,
                category_ids,
            }
        })
        .collect();
    Ok(result)
}

#[tauri::command]
pub async fn overview_counts(state: State<'_, DbState>) -> AppResult<OverviewCounts> {
    let conn = state.0.lock().unwrap();
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))?;
    let uncategorized: i64 = conn.query_row(
        "SELECT COUNT(*) FROM accounts a
         WHERE NOT EXISTS (SELECT 1 FROM account_categories ac WHERE ac.account_id = a.id)",
        [],
        |r| r.get(0),
    )?;

    let mut platforms = Vec::new();
    {
        let mut stmt =
            conn.prepare("SELECT platform, COUNT(*) FROM accounts GROUP BY platform ORDER BY 2 DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(NameCount {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        for r in rows {
            platforms.push(r?);
        }
    }
    let mut interaction = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT interaction_level, COUNT(*) FROM accounts GROUP BY interaction_level",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(NameCount {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        for r in rows {
            interaction.push(r?);
        }
    }

    Ok(OverviewCounts {
        total,
        uncategorized,
        platforms,
        interaction,
    })
}

/// 批量把账号归入分类（追加，不清除已有分类）
#[tauri::command]
pub async fn assign_categories(
    state: State<'_, DbState>,
    account_ids: Vec<String>,
    category_ids: Vec<String>,
) -> AppResult<usize> {
    if account_ids.is_empty() {
        return Err(AppError::Validation("未选择任何账号".into()));
    }
    if category_ids.is_empty() {
        return Err(AppError::Validation("未选择任何分类".into()));
    }
    let mut conn = state.0.lock().unwrap();
    let tx = conn.transaction()?;
    for cid in &category_ids {
        let exists: i64 = tx.query_row(
            "SELECT COUNT(*) FROM categories WHERE id = ?1",
            rusqlite::params![cid],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(AppError::NotFound(format!("分类不存在: {cid}")));
        }
    }
    for aid in &account_ids {
        for cid in &category_ids {
            tx.execute(
                "INSERT OR IGNORE INTO account_categories (account_id, category_id)
                 VALUES (?1, ?2)",
                rusqlite::params![aid, cid],
            )?;
        }
    }
    tx.commit()?;
    Ok(account_ids.len())
}

/// 从指定分类移除账号
#[tauri::command]
pub async fn remove_category_from_accounts(
    state: State<'_, DbState>,
    account_ids: Vec<String>,
    category_id: String,
) -> AppResult<()> {
    let conn = state.0.lock().unwrap();
    for aid in &account_ids {
        conn.execute(
            "DELETE FROM account_categories WHERE account_id = ?1 AND category_id = ?2",
            rusqlite::params![aid, category_id],
        )?;
    }
    Ok(())
}

#[tauri::command]
pub async fn update_note(
    state: State<'_, DbState>,
    id: String,
    note: Option<String>,
) -> AppResult<()> {
    let conn = state.0.lock().unwrap();
    let affected = conn.execute(
        "UPDATE accounts SET note = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![note.filter(|s| !s.trim().is_empty()), chrono::Utc::now().timestamp(), id],
    )?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("账号不存在: {id}")));
    }
    Ok(())
}
