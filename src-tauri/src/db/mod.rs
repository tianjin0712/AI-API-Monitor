//! SQLite 数据库层：连接管理 + 迁移（schema 版本化）

use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// 全局数据库句柄（tauri managed state）。
/// rusqlite::Connection 不是 Sync，故用 Mutex 包裹。
pub struct Db(pub Mutex<Connection>);

impl Db {
    /// 打开（必要时创建）数据库文件并执行迁移。
    pub fn open(db_path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&conn)?;
        Ok(Db(Mutex::new(conn)))
    }

    /// 便捷方法：加锁执行只读闭包。
    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> rusqlite::Result<T> {
        let conn = self.0.lock().expect("db mutex poisoned");
        f(&conn)
    }
}

/// 顺序执行迁移，直到 schema 版本达到 SCHEMA_VERSION。
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let mut version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version < 1 {
        conn.execute_batch(
            r#"
            -- Provider 配置表：API Key 绝不落库，仅存 keyring 引用
            CREATE TABLE IF NOT EXISTS providers (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT    NOT NULL,
                provider_type   TEXT    NOT NULL,           -- deepseek / openai / codex / custom ...
                api_url         TEXT    NOT NULL,
                key_ref         TEXT    NOT NULL,           -- keyring service/account 引用
                enabled         INTEGER NOT NULL DEFAULT 1,
                created_time    TEXT    NOT NULL,
                updated_time    TEXT    NOT NULL
            );

            -- Usage 历史表：用于日报/周报/月报与消耗曲线
            CREATE TABLE IF NOT EXISTS usage_history (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_id  INTEGER NOT NULL,
                date         TEXT    NOT NULL,              -- YYYY-MM-DD
                tokens       INTEGER NOT NULL DEFAULT 0,
                cost         REAL    NOT NULL DEFAULT 0,
                balance      REAL,
                raw_json     TEXT,
                created_time TEXT    NOT NULL,
                FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_usage_provider_date
                ON usage_history (provider_id, date);

            -- 应用设置表（key-value，JSON 序列化）
            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        version = 1;
        conn.pragma_update(None, "user_version", version)?;
    }

    if version < 2 {
        // V2: usage_history 按 (provider_id, date) 唯一，支持当日记录 UPSERT（日报/周报/月报基础）
        conn.execute_batch(
            "DROP INDEX IF EXISTS idx_usage_provider_date;
             CREATE UNIQUE INDEX idx_usage_provider_date ON usage_history (provider_id, date);",
        )?;
        version = 2;
        conn.pragma_update(None, "user_version", version)?;
    }

    if version < 3 {
        // V3: 标记旧凭据（provider_<name>）已迁移到 UUID 引用。
        // 实际数据迁移由 settings::migrate_legacy_credentials 在应用启动时执行（依赖 keyring）。
        version = 3;
        conn.pragma_update(None, "user_version", version)?;
    }

    if version < 4 {
        // V4: usage_history 增加 today_tokens（当日 Token，可空=未知）；
        // cost 允许 NULL（不再把"未提供费用"伪装成 0）——重建表迁移。
        conn.execute_batch(
            "ALTER TABLE usage_history RENAME TO usage_history_old;
             CREATE TABLE usage_history (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_id  INTEGER NOT NULL,
                date         TEXT    NOT NULL,              -- YYYY-MM-DD
                tokens       INTEGER NOT NULL DEFAULT 0,    -- 累计 Token 快照（兼容历史）
                today_tokens INTEGER,                       -- 当日 Token（V0.5，NULL=未知）
                cost         REAL,                          -- 当日费用（V0.5，NULL=未知）
                balance      REAL,
                raw_json     TEXT,
                created_time TEXT    NOT NULL,
                FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
             );
             INSERT INTO usage_history (id, provider_id, date, tokens, today_tokens, cost, balance, raw_json, created_time)
               SELECT id, provider_id, date, tokens, NULL, cost, balance, raw_json, created_time FROM usage_history_old;
             DROP TABLE usage_history_old;
             CREATE UNIQUE INDEX idx_usage_provider_date ON usage_history (provider_id, date);",
        )?;
        version = 4;
        conn.pragma_update(None, "user_version", version)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_creates_tables_and_sets_version() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 4);
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for t in ["providers", "usage_history", "settings"] {
            assert!(tables.iter().any(|x| x == t), "缺少表 {t}");
        }
    }

    #[test]
    fn migration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap(); // 重复迁移不应报错
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 4);
    }
}
