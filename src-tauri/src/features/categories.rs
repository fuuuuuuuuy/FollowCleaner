use crate::db::DbState;
use crate::error::{AppError, AppResult};
use crate::models::Category;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use tauri::State;
use uuid::Uuid;

const MAX_DEPTH: i64 = 5;

fn now() -> i64 {
    Utc::now().timestamp()
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get("id")?,
        name: row.get("name")?,
        parent_id: row.get("parent_id")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// 节点深度（根节点 = 1；id 不存在返回 0）
fn depth_of(conn: &Connection, id: Option<&str>) -> AppResult<i64> {
    let Some(mut current) = id.map(|s| s.to_string()) else {
        return Ok(0);
    };
    let mut depth = 0i64;
    loop {
        depth += 1;
        if depth > 64 {
            return Err(AppError::Db("分类树出现环引用".into()));
        }
        let parent: Option<String> = conn
            .query_row(
                "SELECT parent_id FROM categories WHERE id = ?1",
                params![current],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        match parent {
            Some(p) if !p.is_empty() => current = p,
            _ => break,
        }
    }
    Ok(depth)
}

/// 从 node 向上追溯祖先链，若包含 ancestor_id 则返回 true（node == ancestor 也算）
fn chain_contains(conn: &Connection, node_id: &str, ancestor_id: &str) -> AppResult<bool> {
    let mut current = node_id.to_string();
    loop {
        if current == ancestor_id {
            return Ok(true);
        }
        let parent: Option<String> = conn
            .query_row(
                "SELECT parent_id FROM categories WHERE id = ?1",
                params![current],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        match parent {
            Some(p) if !p.is_empty() => current = p,
            _ => return Ok(false),
        }
    }
}

/// 子树高度（从 root 到其最深后代的边数；叶子为 0）
fn subtree_height(conn: &Connection, root_id: &str) -> AppResult<i64> {
    let mut all: Vec<(String, Option<String>)> = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT id, parent_id FROM categories")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?;
        for r in rows {
            all.push(r?);
        }
    }
    fn height_of(
        id: &str,
        all: &[(String, Option<String>)],
        memo: &mut HashMap<String, i64>,
        guard: &mut i64,
    ) -> i64 {
        if let Some(h) = memo.get(id) {
            return *h;
        }
        *guard += 1;
        if *guard > 10_000 {
            return 0; // 防御性
        }
        let mut max_child = -1i64;
        for (cid, pid) in all {
            if pid.as_deref() == Some(id) {
                max_child = max_child.max(height_of(cid, all, memo, guard));
            }
        }
        let h = max_child + 1;
        memo.insert(id.to_string(), h);
        h
    }
    let mut memo = HashMap::new();
    let mut guard = 0i64;
    Ok(height_of(root_id, &all, &mut memo, &mut guard))
}

fn sibling_name_taken(
    conn: &Connection,
    name: &str,
    parent_id: Option<&str>,
    exclude_id: Option<&str>,
) -> AppResult<bool> {
    let sql = match (parent_id, exclude_id) {
        (Some(_), Some(_)) => {
            "SELECT COUNT(*) FROM categories WHERE name = ?1 AND parent_id = ?2 AND id != ?3"
        }
        (Some(_), None) => {
            "SELECT COUNT(*) FROM categories WHERE name = ?1 AND parent_id = ?2"
        }
        (None, Some(_)) => {
            "SELECT COUNT(*) FROM categories WHERE name = ?1 AND parent_id IS NULL AND id != ?2"
        }
        (None, None) => "SELECT COUNT(*) FROM categories WHERE name = ?1 AND parent_id IS NULL",
    };
    let count: i64 = match (parent_id, exclude_id) {
        (Some(p), Some(ex)) => conn.query_row(sql, params![name, p, ex], |r| r.get(0))?,
        (Some(p), None) => conn.query_row(sql, params![name, p], |r| r.get(0))?,
        (None, Some(ex)) => conn.query_row(sql, params![name, ex], |r| r.get(0))?,
        (None, None) => conn.query_row(sql, params![name], |r| r.get(0))?,
    };
    Ok(count > 0)
}

// ==================== Tauri commands ====================

#[tauri::command]
pub async fn list_categories(state: State<'_, DbState>) -> AppResult<Vec<Category>> {
    let conn = state.0.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT * FROM categories
         ORDER BY COALESCE(parent_id, '') ASC, sort_order ASC, created_at ASC",
    )?;
    let rows = stmt.query_map([], map_row)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 每个分类下直接归属的账号数量
#[tauri::command]
pub async fn category_counts(state: State<'_, DbState>) -> AppResult<HashMap<String, i64>> {
    let conn = state.0.lock().unwrap();
    let mut stmt =
        conn.prepare("SELECT category_id, COUNT(*) AS n FROM account_categories GROUP BY category_id")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut map = HashMap::new();
    for r in rows {
        let (id, n) = r?;
        map.insert(id, n);
    }
    Ok(map)
}

#[tauri::command]
pub async fn create_category(
    state: State<'_, DbState>,
    name: String,
    parent_id: Option<String>,
) -> AppResult<Category> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("分类名称不能为空".into()));
    }
    let conn = state.0.lock().unwrap();
    let parent = parent_id.filter(|s| !s.is_empty());

    if let Some(pid) = &parent {
        let exists: i64 =
            conn.query_row("SELECT COUNT(*) FROM categories WHERE id = ?1", params![pid], |r| {
                r.get(0)
            })?;
        if exists == 0 {
            return Err(AppError::NotFound(format!("父分类不存在: {pid}")));
        }
    }
    if depth_of(&conn, parent.as_deref())? + 1 > MAX_DEPTH {
        return Err(AppError::CategoryDepth);
    }
    if sibling_name_taken(&conn, &name, parent.as_deref(), None)? {
        return Err(AppError::DuplicateName);
    }

    let id = Uuid::new_v4().to_string();
    let ts = now();
    let next_order: i64 = match &parent {
        Some(p) => conn.query_row(
            "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM categories WHERE parent_id = ?1",
            params![p],
            |r| r.get(0),
        )?,
        None => conn.query_row(
            "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM categories WHERE parent_id IS NULL",
            [],
            |r| r.get(0),
        )?,
    };

    conn.execute(
        "INSERT INTO categories (id, name, parent_id, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, parent, next_order, ts, ts],
    )?;

    Ok(Category {
        id,
        name,
        parent_id: parent,
        sort_order: next_order,
        created_at: ts,
        updated_at: ts,
    })
}

#[tauri::command]
pub async fn rename_category(state: State<'_, DbState>, id: String, name: String) -> AppResult<()> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("分类名称不能为空".into()));
    }
    let conn = state.0.lock().unwrap();
    let current: Option<Category> = conn
        .query_row("SELECT * FROM categories WHERE id = ?1", params![id], map_row)
        .optional()?;
    let Some(current) = current else {
        return Err(AppError::NotFound(format!("分类不存在: {id}")));
    };
    if sibling_name_taken(&conn, &name, current.parent_id.as_deref(), Some(&id))? {
        return Err(AppError::DuplicateName);
    }
    conn.execute(
        "UPDATE categories SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, now(), id],
    )?;
    Ok(())
}

#[tauri::command]
pub async fn delete_category(state: State<'_, DbState>, id: String) -> AppResult<()> {
    let conn = state.0.lock().unwrap();
    let child_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM categories WHERE parent_id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if child_count > 0 {
        return Err(AppError::HasChildren);
    }
    // account_categories 通过 ON DELETE CASCADE 自动解除关联（账号进入"未分类"）
    conn.execute("DELETE FROM categories WHERE id = ?1", params![id])?;
    Ok(())
}

/// 移动分类到新父级，并插入到新兄弟组的 position 位置（从 0 开始）
#[tauri::command]
pub async fn move_category(
    state: State<'_, DbState>,
    id: String,
    new_parent_id: Option<String>,
    position: i64,
) -> AppResult<()> {
    let new_parent = new_parent_id.filter(|s| !s.is_empty());
    let mut conn = state.0.lock().unwrap();
    let tx = conn.transaction()?;

    // 存在性
    let exists: i64 = tx.query_row(
        "SELECT COUNT(*) FROM categories WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::NotFound(format!("分类不存在: {id}")));
    }
    if let Some(pid) = &new_parent {
        let p_exists: i64 = tx.query_row(
            "SELECT COUNT(*) FROM categories WHERE id = ?1",
            params![pid],
            |r| r.get(0),
        )?;
        if p_exists == 0 {
            return Err(AppError::NotFound(format!("父分类不存在: {pid}")));
        }
        // 新父级不能是自身或自己的后代
        if pid == &id || chain_contains(&tx, pid, &id)? {
            return Err(AppError::CategoryCycle);
        }
    }

    // 深度校验：移动后子树最大深度不得超过 MAX_DEPTH
    let new_depth = depth_of(&tx, new_parent.as_deref())? + 1;
    let height = subtree_height(&tx, &id)?;
    if new_depth + height > MAX_DEPTH {
        return Err(AppError::CategoryDepth);
    }

    // 1) 更新父级
    tx.execute(
        "UPDATE categories SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_parent, now(), id],
    )?;

    // 2) 重排新兄弟组（每臂各自收集为 Vec，避免闭包类型不一致）
    let mut siblings: Vec<String> = {
        let mut stmt = match &new_parent {
            Some(_) => tx.prepare(
                "SELECT id FROM categories WHERE parent_id = ?1 AND id != ?2
                 ORDER BY sort_order ASC",
            )?,
            None => tx.prepare(
                "SELECT id FROM categories WHERE parent_id IS NULL AND id != ?1
                 ORDER BY sort_order ASC",
            )?,
        };
        let collected: rusqlite::Result<Vec<String>> = match &new_parent {
            Some(p) => stmt
                .query_map(params![p, id], |r| r.get::<_, String>(0))?
                .collect(),
            None => stmt
                .query_map(params![id], |r| r.get::<_, String>(0))?
                .collect(),
        };
        collected?
    };
    let pos = position.clamp(0, siblings.len() as i64) as usize;
    siblings.insert(pos, id.clone());
    for (idx, sid) in siblings.iter().enumerate() {
        tx.execute(
            "UPDATE categories SET sort_order = ?1 WHERE id = ?2",
            params![idx as i64, sid],
        )?;
    }

    tx.commit()?;
    Ok(())
}
