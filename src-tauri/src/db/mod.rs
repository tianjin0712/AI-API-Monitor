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
    pub fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
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

    #[test]
    fn migration_v3_to_v4_preserves_data_and_marks_today_null() {
        // 手动构造 V3 状态（旧 usage_history 结构 + 数据），验证 V4 迁移保数据
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA user_version = 3;
             CREATE TABLE providers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                api_url TEXT NOT NULL,
                key_ref TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_time TEXT NOT NULL,
                updated_time TEXT NOT NULL
             );
             INSERT INTO providers (name, provider_type, api_url, key_ref, enabled, created_time, updated_time)
               VALUES ('deepseek', 'deepseek', 'https://api.deepseek.com', 'key_x', 1, 't', 't');
             CREATE TABLE usage_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_id INTEGER NOT NULL,
                date TEXT NOT NULL,
                tokens INTEGER NOT NULL DEFAULT 0,
                cost REAL NOT NULL DEFAULT 0,
                balance REAL,
                raw_json TEXT,
                created_time TEXT NOT NULL
             );
             CREATE UNIQUE INDEX idx_usage_provider_date ON usage_history (provider_id, date);
             INSERT INTO usage_history (provider_id, date, tokens, cost, balance, raw_json, created_time)
               VALUES (1, '2025-08-01', 1000, 1.25, 42.0, '{}', 't');",
        )
        .unwrap();

        migrate(&conn).unwrap();

        // 版本推进到 4
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 4);
        // 数据保留：tokens/cost/balance 原样，today_tokens 为 NULL（未知）
        let (tokens, today_tokens, cost, balance): (i64, Option<i64>, f64, f64) = conn
            .query_row(
                "SELECT tokens, today_tokens, cost, balance FROM usage_history WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(tokens, 1000);
        assert_eq!(today_tokens, None);
        assert!((cost - 1.25).abs() < 1e-9);
        assert!((balance - 42.0).abs() < 1e-9);
        // 唯一索引存在（V4 重建）
        let idx: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_usage_provider_date'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx, 1);
        // 旧表已清理
        let old: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='usage_history_old'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(old, 0);
    }

    #[test]
    fn migration_v3_to_v4_preserves_foreign_key_cascade() {
        // V3 旧表含外键 ON DELETE CASCADE；迁移后级联删除仍生效
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(
            "PRAGMA user_version = 3;
             CREATE TABLE providers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                api_url TEXT NOT NULL,
                key_ref TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_time TEXT NOT NULL,
                updated_time TEXT NOT NULL
             );
             INSERT INTO providers (name, provider_type, api_url, key_ref, enabled, created_time, updated_time)
               VALUES ('openai', 'openai', 'https://api.openai.com/v1', 'key_y', 1, 't', 't');
             CREATE TABLE usage_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_id INTEGER NOT NULL,
                date TEXT NOT NULL,
                tokens INTEGER NOT NULL DEFAULT 0,
                cost REAL NOT NULL DEFAULT 0,
                balance REAL,
                raw_json TEXT,
                created_time TEXT NOT NULL,
                FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
             );
             CREATE UNIQUE INDEX idx_usage_provider_date ON usage_history (provider_id, date);
             INSERT INTO usage_history (provider_id, date, tokens, cost, balance, raw_json, created_time)
               VALUES (1, '2025-08-01', 100, 0.5, 10.0, '{}', 't'),
                      (1, '2025-08-02', 200, 1.0, 9.0, '{}', 't');",
        )
        .unwrap();

        migrate(&conn).unwrap();
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            4
        );

        // 删除 provider，usage_history 应级联清空
        conn.execute("DELETE FROM providers WHERE id = 1", [])
            .unwrap();
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM usage_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0, "迁移后外键 ON DELETE CASCADE 应仍然生效");
    }
}
