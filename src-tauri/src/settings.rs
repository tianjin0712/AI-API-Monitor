//! Settings 系统：Provider 增删改查 + 应用设置（key-value）持久化
//!
//! - Provider 配置落 SQLite，API Key 经 [`crate::storage::SecureStorage`] 入系统凭据库
//! - 刷新策略（mission.md §12）：前台 10s / 后台 60s，存 settings 表

use crate::db::Db;
use crate::providers::ProviderConfig;
use crate::storage::SecureStorage;
use chrono::Utc;
use rusqlite::OptionalExtension;

/// settings 表键名：前台刷新间隔（秒）
pub const SETTING_REFRESH_FOREGROUND_SECS: &str = "refresh.foregroundSecs";
/// settings 表键名：后台刷新间隔（秒）
pub const SETTING_REFRESH_BACKGROUND_SECS: &str = "refresh.backgroundSecs";

/// 应用层错误，统一映射为前端可读信息。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("密钥存储错误: {0}")]
    Storage(#[from] crate::storage::StorageError),
    #[error("Provider 不存在: id={0}")]
    ProviderNotFound(i64),
    #[error("参数错误: {0}")]
    Invalid(String),
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

/// 列出全部 Provider（不含 API Key）。
pub fn list_providers(db: &Db) -> Result<Vec<ProviderConfig>, AppError> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, provider_type, api_url, key_ref, enabled, created_time, updated_time
             FROM providers ORDER BY id",
        )?;
        let rows = stmt.query_map([], row_to_provider)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    })
    .map_err(AppError::from)
}

/// 新增 Provider：API Key 先入 keyring，再写数据库。
#[allow(clippy::too_many_arguments)]
pub fn add_provider(
    db: &Db,
    name: &str,
    provider_type: &str,
    api_url: &str,
    api_key: &str,
) -> Result<ProviderConfig, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Invalid("名称不能为空".into()));
    }
    if provider_type.is_empty() {
        return Err(AppError::Invalid("Provider 类型不能为空".into()));
    }
    if api_key.is_empty() {
        return Err(AppError::Invalid("API Key 不能为空".into()));
    }
    let key_ref = SecureStorage::save_api_key(name, api_key)?;
    let now = Utc::now().to_rfc3339();
    let id = db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO providers (name, provider_type, api_url, key_ref, enabled, created_time, updated_time)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
            rusqlite::params![name, provider_type, api_url, key_ref, now],
        )?;
        Ok(conn.last_insert_rowid())
    })?;
    Ok(ProviderConfig {
        id,
        name: name.to_string(),
        provider_type: provider_type.to_string(),
        api_url: api_url.to_string(),
        key_ref,
        enabled: true,
        created_time: now.clone(),
        updated_time: now,
    })
}

/// 更新 Provider（名称 / API URL / 可选 API Key）。
pub fn update_provider(
    db: &Db,
    id: i64,
    name: &str,
    api_url: &str,
    api_key: Option<&str>,
) -> Result<ProviderConfig, AppError> {
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        let updated = conn.execute(
            "UPDATE providers SET name = ?1, api_url = ?2, updated_time = ?3 WHERE id = ?4",
            rusqlite::params![name, api_url, now, id],
        )?;
        if updated == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::ProviderNotFound(id),
        other => AppError::Db(other),
    })?;
    // 更新密钥（可选）
    if let Some(key) = api_key {
        if !key.is_empty() {
            let key_ref: String = db.with_conn(|conn| {
                conn.query_row(
                    "SELECT key_ref FROM providers WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
            })?;
            SecureStorage::update_api_key(&key_ref, key)?;
        }
    }
    // 返回更新后的完整记录
    db.with_conn(|conn| {
        conn.query_row(
            "SELECT id, name, provider_type, api_url, key_ref, enabled, created_time, updated_time
             FROM providers WHERE id = ?1",
            [id],
            row_to_provider,
        )
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::ProviderNotFound(id),
        other => AppError::Db(other),
    })
}

/// 删除 Provider：先删数据库行，再清 keyring 凭证。
pub fn delete_provider(db: &Db, id: i64) -> Result<(), AppError> {
    let key_ref: Option<String> = db.with_conn(|conn| {
        conn.query_row(
            "SELECT key_ref FROM providers WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
    })?;
    db.with_conn(|conn| {
        let deleted = conn.execute("DELETE FROM providers WHERE id = ?1", [id])?;
        if deleted == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(())
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::ProviderNotFound(id),
        other => AppError::Db(other),
    })?;
    if let Some(kr) = key_ref {
        // 凭据可能已被删除/不存在，忽略清理失败
        let _ = SecureStorage::delete_api_key(&kr);
    }
    Ok(())
}

/// 读取设置值。
pub fn get_setting(db: &Db, key: &str) -> Result<Option<String>, AppError> {
    Ok(db.with_conn(|conn| {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
    })?)
}

/// 写入/覆盖设置值。
pub fn set_setting(db: &Db, key: &str, value: &str) -> Result<(), AppError> {
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )?;
        Ok(())
    })
    .map_err(AppError::from)
}

/// 读取前台刷新间隔（秒），未配置时返回默认值 10。
pub fn refresh_foreground_secs(db: &Db) -> Result<u64, AppError> {
    Ok(get_setting(db, SETTING_REFRESH_FOREGROUND_SECS)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(10))
}

/// 读取后台刷新间隔（秒），未配置时返回默认值 60。
pub fn refresh_background_secs(db: &Db) -> Result<u64, AppError> {
    Ok(get_setting(db, SETTING_REFRESH_BACKGROUND_SECS)?
        .and_then(|v| v.parse().ok())
        .unwrap_or(60))
}

/// 行 -> ProviderConfig 映射。
fn row_to_provider(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderConfig> {
    Ok(ProviderConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        provider_type: row.get(2)?,
        api_url: row.get(3)?,
        key_ref: row.get(4)?,
        enabled: row.get::<_, i64>(5)? != 0,
        created_time: row.get(6)?,
        updated_time: row.get(7)?,
    })
}
