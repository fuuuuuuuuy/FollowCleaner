use crate::error::AppResult;
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// 全局共享数据库连接（Tauri State 管理）
pub struct DbState(pub Mutex<Connection>);

/// 迁移列表：每个元素为一个版本的 SQL（可含多条语句）
const MIGRATIONS: &[&str] = &[
    // ---- v1: V0.1 初始 schema ----
    r#"
    CREATE TABLE accounts (
      id                  TEXT PRIMARY KEY,
      platform            TEXT NOT NULL,
      platform_account_id TEXT NOT NULL,
      display_name        TEXT NOT NULL,
      avatar_path         TEXT,
      account_type        TEXT NOT NULL DEFAULT 'unknown',
      homepage_url        TEXT,
      followed_at         INTEGER,
      interaction_level   TEXT NOT NULL DEFAULT 'unknown',
      interaction_metrics TEXT,
      status              TEXT NOT NULL DEFAULT 'following',
      note                TEXT,
      imported_at         INTEGER NOT NULL,
      updated_at          INTEGER NOT NULL,
      UNIQUE(platform, platform_account_id)
    );

    CREATE TABLE categories (
      id         TEXT PRIMARY KEY,
      name       TEXT NOT NULL,
      parent_id  TEXT REFERENCES categories(id) ON DELETE SET NULL,
      sort_order INTEGER NOT NULL DEFAULT 0,
      created_at INTEGER NOT NULL,
      updated_at INTEGER NOT NULL
    );

    CREATE TABLE account_categories (
      account_id  TEXT NOT NULL REFERENCES accounts(id)   ON DELETE CASCADE,
      category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
      PRIMARY KEY (account_id, category_id)
    );

    CREATE INDEX idx_accounts_platform     ON accounts(platform);
    CREATE INDEX idx_accounts_interaction  ON accounts(interaction_level);
    CREATE INDEX idx_accounts_followed     ON accounts(followed_at);
    CREATE INDEX idx_accounts_status       ON accounts(status);
    CREATE INDEX idx_accounts_name         ON accounts(display_name);
    CREATE INDEX idx_ac_category           ON account_categories(category_id);
    "#,
    // ---- v2: 平台会话凭证（L2 网页登录方式；仅存本机） ----
    r#"
    CREATE TABLE credentials (
      platform     TEXT PRIMARY KEY,
      cookie_json  TEXT NOT NULL,
      account_name TEXT,
      created_at   INTEGER NOT NULL,
      updated_at   INTEGER NOT NULL
    );
    "#,
];

/// 打开（必要时创建）数据库并执行未应用的迁移
pub fn open_and_migrate(app_data_dir: &PathBuf) -> AppResult<Connection> {
    fs::create_dir_all(app_data_dir)?;
    let db_path = app_data_dir.join("followcleaner.db");
    log::info!("opening database at {}", db_path.display());

    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let current: i64 =
        conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| {
            r.get(0)
        })?;

    for (idx, sql) in MIGRATIONS.iter().enumerate() {
        let version = (idx as i64) + 1;
        if version > current {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![version, chrono::Utc::now().timestamp()],
            )?;
            tx.commit()?;
            log::info!("migration v{version} applied");
        }
    }

    Ok(conn)
}
