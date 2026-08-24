//! SQLite 数据库层：连接管理 + 迁移（schema 版本化）

use rusqlite::{Connection, Error as SqlError, ErrorCode};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const SCHEMA_VERSION: i64 = 7;
const OPEN_RETRIES: usize = 8;
const SNAPSHOT_LIMIT: usize = 5;

/// 全局数据库句柄（tauri managed state）。
/// rusqlite::Connection 不是 Sync，故用 Mutex 包裹。
pub struct Db {
    pub(crate) conn: Mutex<Connection>,
    pub(crate) recovery_notice: Option<String>,
}

/// 数据库初始化失败错误：保留底层 rusqlite 错误作为 source，同时标注失败步骤，
/// 使启动期日志能够区分「打开连接 / busy_timeout / 权限 / WAL / 外键 / schema 检查 / 快照 / 迁移」等阶段。
#[derive(Debug)]
pub(crate) struct InitError {
    step: &'static str,
    source: SqlError,
}

impl InitError {
    fn at(step: &'static str, source: SqlError) -> Self {
        Self { step, source }
    }

    /// 底层 SQLite 错误，供 is_busy / is_corruption 分类判断。
    fn sql_error(&self) -> &SqlError {
        &self.source
    }
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.step, self.source)
    }
}

impl std::error::Error for InitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl Db {
    /// 打开（必要时创建）数据库文件并执行迁移。
    pub fn open(db_path: &Path) -> Result<Self, InitError> {
        let mut last_error = None;
        for attempt in 0..OPEN_RETRIES {
            match open_once(db_path) {
                Ok(conn) => {
                    return Ok(Self {
                        conn: Mutex::new(conn),
                        recovery_notice: None,
                    });
                }
                Err(error) if is_busy(error.sql_error()) && attempt + 1 < OPEN_RETRIES => {
                    crate::security::safe_log(
                        "database_retry",
                        format!("database locked, retry {}/{}", attempt + 1, OPEN_RETRIES),
                    );
                    thread::sleep(Duration::from_millis(250 * (attempt as u64 + 1)));
                    last_error = Some(error);
                }
                Err(error) => {
                    last_error = Some(error);
                    break;
                }
            }
        }

        let error = last_error.expect("database open must produce an error");
        crate::security::safe_log(
            "database_open_failed",
            format!(
                "database open failed; path={}; step={}; class={}; error={}",
                db_path.display(),
                error.step,
                database_error_class(error.sql_error()),
                error.sql_error()
            ),
        );
        if is_busy(error.sql_error()) {
            crate::security::safe_log("database_locked", "database remained locked after retries");
        }
        if !db_path.exists() || is_busy(error.sql_error()) || !is_corruption(error.sql_error()) {
            return Err(error);
        }

        // SQLite WAL state belongs to its main database. Move both sidecars
        // before creating a replacement, so a fresh ai-api-monitor.db can
        // never replay an old -wal file. If preserving any file fails, do not
        // create a replacement database: returning the error is safer than
        // risking a partial recovery or data loss.
        let recovery_path = recovery_path(db_path);
        crate::security::safe_log(
            "database_recovery_started",
            format!(
                "database recovery started; original preserved at {}",
                recovery_path.display()
            ),
        );
        if let Err(error) = preserve_corrupt_database(db_path, &recovery_path) {
            crate::security::safe_log(
                "database_recovery_failed",
                format!("could not preserve corrupt database; error={error}"),
            );
            return Err(InitError::at("保留损坏数据库副本", error));
        }
        let conn = match open_fresh(db_path) {
            Ok(conn) => conn,
            Err(error) => {
                crate::security::safe_log(
                    "database_recovery_failed",
                    format!(
                        "could not initialize clean database; step={}; error={}",
                        error.step,
                        error.sql_error()
                    ),
                );
                return Err(error);
            }
        };
        crate::security::safe_log(
            "database_recovery_completed",
            "database recovery completed with a clean database",
        );
        Ok(Self {
            conn: Mutex::new(conn),
            recovery_notice: Some(format!(
                "原数据库无法安全打开，已保留在 {}；应用已使用安全空数据库启动，请检查并恢复数据。",
                recovery_path.display()
            )),
        })
    }

    pub fn recovery_notice(&self) -> Option<&str> {
        self.recovery_notice.as_deref()
    }

    /// 便捷方法：加锁执行只读闭包。
    pub fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        f(&conn)
    }
}

fn open_once(db_path: &Path) -> Result<Connection, InitError> {
    let conn = Connection::open(db_path).map_err(|error| InitError::at("打开数据库连接", error))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|error| InitError::at("设置 busy_timeout", error))?;
    crate::platform_security::harden_private_path(db_path, false)
        .map_err(io_error)
        .map_err(|error| InitError::at("加固数据库文件权限", error))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|error| InitError::at("设置 WAL 日志模式", error))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| InitError::at("启用外键约束", error))?;
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| InitError::at("读取 schema 版本 (PRAGMA user_version)", error))?;
    if version < SCHEMA_VERSION {
        // Flush as much WAL state as possible before copying the snapshot;
        // the sidecar files are copied as well when another process still
        // keeps the database in WAL mode.
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE)");
        create_snapshot(db_path, version)
            .map_err(|error| InitError::at("创建迁移前快照", error))?;
    }
    migrate(&conn).map_err(|error| InitError::at("执行数据库迁移", error))?;
    Ok(conn)
}

fn open_fresh(db_path: &Path) -> Result<Connection, InitError> {
    let conn = Connection::open(db_path).map_err(|error| InitError::at("打开数据库连接", error))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|error| InitError::at("设置 busy_timeout", error))?;
    crate::platform_security::harden_private_path(db_path, false)
        .map_err(io_error)
        .map_err(|error| InitError::at("加固数据库文件权限", error))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|error| InitError::at("设置 WAL 日志模式", error))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| InitError::at("启用外键约束", error))?;
    migrate(&conn).map_err(|error| InitError::at("执行数据库迁移", error))?;
    Ok(conn)
}

fn io_error(error: std::io::Error) -> SqlError {
    SqlError::ToSqlConversionFailure(Box::new(error))
}

fn is_busy(error: &SqlError) -> bool {
    matches!(
        error,
        SqlError::SqliteFailure(inner, _) if matches!(inner.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

/// Only these SQLite failures mean the database bytes cannot be safely read.
/// In particular, filesystem, permission, migration, and I/O errors must not
/// trigger recovery: the original database remains in place for investigation.
fn is_corruption(error: &SqlError) -> bool {
    matches!(
        error,
        SqlError::SqliteFailure(inner, _) if matches!(
            inner.code,
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase
        )
    )
}

fn database_error_class(error: &SqlError) -> &'static str {
    if is_busy(error) {
        "locked_or_busy"
    } else if is_corruption(error) {
        "corruption"
    } else {
        "other"
    }
}

fn recovery_path(db_path: &Path) -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let unique = uuid::Uuid::new_v4();
    PathBuf::from(format!(
        "{}.recovery-{}-{unique}.db",
        db_path.display(),
        stamp
    ))
}

fn preserve_corrupt_database(db_path: &Path, recovery_path: &Path) -> rusqlite::Result<()> {
    // Move the main file first. If a sidecar move fails, restore everything we
    // already moved and abort recovery; a replacement database must never be
    // created from a partially preserved WAL set.
    std::fs::rename(db_path, recovery_path).map_err(io_error)?;
    let mut moved_sidecars = Vec::new();
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", db_path.display(), suffix));
        if sidecar.exists() {
            let recovered_sidecar = PathBuf::from(format!("{}{}", recovery_path.display(), suffix));
            if let Err(error) = std::fs::rename(&sidecar, &recovered_sidecar) {
                for (original, preserved) in moved_sidecars.iter().rev() {
                    let _ = std::fs::rename(preserved, original);
                }
                let _ = std::fs::rename(recovery_path, db_path);
                return Err(io_error(error));
            }
            moved_sidecars.push((sidecar, recovered_sidecar));
        }
    }
    Ok(())
}

fn create_snapshot(db_path: &Path, version: i64) -> rusqlite::Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    let parent = db_path.parent().unwrap_or_else(|| Path::new("."));
    let snapshot_dir = parent.join("migration-snapshots");
    std::fs::create_dir_all(&snapshot_dir).map_err(io_error)?;
    crate::platform_security::harden_private_path(&snapshot_dir, true).map_err(io_error)?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S%.3f");
    let base = snapshot_dir.join(format!(
        "ai-api-monitor-v{version}-{stamp}-{}.db",
        uuid::Uuid::new_v4()
    ));
    std::fs::copy(db_path, &base).map_err(io_error)?;
    for suffix in ["-wal", "-shm"] {
        let source = PathBuf::from(format!("{}{}", db_path.display(), suffix));
        if source.exists() {
            let target = PathBuf::from(format!("{}{}", base.display(), suffix));
            std::fs::copy(source, target).map_err(io_error)?;
        }
    }
    crate::platform_security::harden_private_path(&base, false).map_err(io_error)?;
    crate::security::safe_log(
        "migration",
        format!("snapshot created for schema v{version}"),
    );
    retain_snapshots(&snapshot_dir);
    Ok(())
}

fn retain_snapshots(dir: &Path) {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".db"))
        .collect();
    files.sort_by_key(|entry| entry.file_name());
    while files.len() > SNAPSHOT_LIMIT {
        if let Some(entry) = files.first() {
            let path = entry.path();
            let _ = std::fs::remove_file(&path);
            for suffix in ["-wal", "-shm"] {
                let _ =
                    std::fs::remove_file(PathBuf::from(format!("{}{}", path.display(), suffix)));
            }
        }
        files.remove(0);
    }
}

/// 顺序执行迁移，直到 schema 版本达到 SCHEMA_VERSION。
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    crate::security::safe_log("migration", "migration start");
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = migrate_inner(conn);
    match result {
        Ok(()) => {
            let result = conn.execute_batch("COMMIT");
            if result.is_ok() {
                crate::security::safe_log(
                    "migration",
                    format!("migration success schema_v={SCHEMA_VERSION}"),
                );
            }
            result
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            crate::security::safe_log(
                "migration",
                format!("migration failed and rolled back: {error}"),
            );
            Err(error)
        }
    }
}

fn migrate_inner(conn: &Connection) -> rusqlite::Result<()> {
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

    if version < 5 {
        conn.execute_batch("ALTER TABLE providers ADD COLUMN key_hint TEXT NOT NULL DEFAULT '';")?;
        version = 5;
        conn.pragma_update(None, "user_version", version)?;
    }

    if version < 6 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS secure_settings (
                key        TEXT PRIMARY KEY,
                nonce      BLOB NOT NULL,
                ciphertext BLOB NOT NULL
             );",
        )?;
        version = 6;
        conn.pragma_update(None, "user_version", version)?;
    }

    if version < 7 {
        // V7: providers 增加 custom_config（通用自定义 API 的非敏感配置 JSON；可为空）。
        conn.execute_batch("ALTER TABLE providers ADD COLUMN custom_config TEXT;")?;
        version = 7;
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
        assert_eq!(version, 7);
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for t in ["providers", "usage_history", "settings", "secure_settings"] {
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
        assert_eq!(version, 7);
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
        assert_eq!(version, 7);
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
            7
        );

        // 删除 provider，usage_history 应级联清空
        conn.execute("DELETE FROM providers WHERE id = 1", [])
            .unwrap();
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM usage_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0, "迁移后外键 ON DELETE CASCADE 应仍然生效");
    }

    #[test]
    fn corrupt_database_is_preserved_and_recovered() {
        let root =
            std::env::temp_dir().join(format!("ai-monitor-db-recovery-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ai-api-monitor.db");
        let original = b"INVALID_SQLITE_DATABASE";
        std::fs::write(&path, original).unwrap();

        let db = Db::open(&path).expect("corrupt database should recover into a clean database");
        assert!(db.recovery_notice().is_some());
        assert_eq!(
            db.with_conn(
                |conn| conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            )
            .unwrap(),
            SCHEMA_VERSION
        );
        let recovery = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                let file_name = path.file_name().unwrap().to_string_lossy();
                file_name.contains(".recovery-") && file_name.ends_with(".db")
            })
            .expect("原始数据库必须保留为 recovery 文件");
        assert_eq!(std::fs::read(&recovery).unwrap(), original);
        assert!(!std::fs::read(&path).unwrap().starts_with(original));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preserve_corrupt_database_moves_existing_sidecars() {
        let root = std::env::temp_dir().join(format!(
            "ai-monitor-db-preserve-sidecars-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ai-api-monitor.db");
        let recovery = root.join("recovery.db");
        std::fs::write(&path, b"database").unwrap();
        std::fs::write(format!("{}-wal", path.display()), b"old wal").unwrap();
        std::fs::write(format!("{}-shm", path.display()), b"old shm").unwrap();

        preserve_corrupt_database(&path, &recovery).unwrap();

        assert_eq!(std::fs::read(&recovery).unwrap(), b"database");
        assert_eq!(
            std::fs::read(format!("{}-wal", recovery.display())).unwrap(),
            b"old wal"
        );
        assert_eq!(
            std::fs::read(format!("{}-shm", recovery.display())).unwrap(),
            b"old shm"
        );
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_paths_are_unique() {
        let path = Path::new("/tmp/ai-api-monitor.db");
        assert_ne!(recovery_path(path), recovery_path(path));
    }

    #[test]
    fn locked_errors_are_not_corruption() {
        let error = SqlError::SqliteFailure(
            rusqlite::ffi::Error {
                code: ErrorCode::DatabaseLocked,
                extended_code: rusqlite::ffi::SQLITE_LOCKED,
            },
            None,
        );
        assert!(is_busy(&error));
        assert!(!is_corruption(&error));
        assert_eq!(database_error_class(&error), "locked_or_busy");
    }

    #[test]
    fn init_error_display_includes_step_and_source() {
        let error = SqlError::SqliteFailure(
            rusqlite::ffi::Error {
                code: ErrorCode::NotADatabase,
                extended_code: rusqlite::ffi::SQLITE_NOTADB,
            },
            Some("file is not a database".into()),
        );
        let init = InitError::at("读取 schema 版本 (PRAGMA user_version)", error);
        let text = init.to_string();
        assert!(
            text.contains("读取 schema 版本"),
            "缺少失败步骤上下文: {text}"
        );
        assert!(
            text.contains("file is not a database"),
            "缺少失败原因: {text}"
        );
        // source 链保留，供上层 is_corruption 分类判断
        assert!(is_corruption(init.sql_error()));
    }
}
