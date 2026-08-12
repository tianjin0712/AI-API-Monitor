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
/// settings 键名：后台刷新间隔（秒）
pub const SETTING_REFRESH_BACKGROUND_SECS: &str = "refresh.backgroundSecs";
/// settings 键名：DIY 布局 JSON（V0.3，含 theme 与 widgets）
pub const SETTING_LAYOUT: &str = "ui.layout";

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

/// 校验 Provider 输入：类型白名单 + URL 合法性（修复 P2）。
/// 官方/自定义 Provider 均要求 HTTPS；仅允许本机回环地址使用 HTTP（本地/自托管调试）。
pub fn validate_provider_input(
    manager: &crate::providers::ProviderManager,
    provider_type: &str,
    api_url: &str,
) -> Result<(), AppError> {
    if manager.get(provider_type).is_none() {
        return Err(AppError::Invalid(format!(
            "不支持的 Provider 类型: {provider_type}"
        )));
    }
    // P0：Codex 复用本机 CLI 凭证，必须使用固定官方地址，禁止任意 host/端口/路径
    if provider_type == "codex" {
        if api_url.trim_end_matches('/') != crate::providers::codex::DEFAULT_CODEX_BASE {
            return Err(AppError::Invalid(
                "Codex 使用固定官方地址，不可修改（防止本机凭证泄露）".into(),
            ));
        }
        return Ok(());
    }
    let parsed =
        url::Url::parse(api_url).map_err(|_| AppError::Invalid("API URL 格式无效".into()))?;
    match parsed.scheme() {
        "https" => Ok(()),
        "http" => {
            let host = parsed.host_str().unwrap_or("");
            if host == "localhost" || host == "127.0.0.1" || host == "::1" {
                Ok(())
            } else {
                Err(AppError::Invalid(
                    "仅允许 HTTPS 地址；HTTP 仅限本机回环地址（localhost）".into(),
                ))
            }
        }
        _ => Err(AppError::Invalid("仅支持 https:// 或 http:// 地址".into())),
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
    manager: &crate::providers::ProviderManager,
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
    if api_key.is_empty() && provider_type != "codex" {
        return Err(AppError::Invalid("API Key 不能为空".into()));
    }
    validate_provider_input(manager, provider_type, api_url)?;
    // Codex 复用 CLI 本地凭证（~/.codex/auth.json），不写入 keyring，key_ref 为空
    let key_ref = if api_key.is_empty() {
        String::new()
    } else {
        let key_id = SecureStorage::gen_key_id();
        SecureStorage::save_api_key(&key_id, api_key)?
    };
    let now = Utc::now().to_rfc3339();
    let insert = db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO providers (name, provider_type, api_url, key_ref, enabled, created_time, updated_time)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)",
            rusqlite::params![name, provider_type, api_url, key_ref, now],
        )?;
        Ok(conn.last_insert_rowid())
    });
    let id = match insert {
        Ok(id) => id,
        Err(e) => {
            // 补偿：数据库写入失败时回滚已保存的凭据，避免孤儿 key
            let _ = SecureStorage::delete_api_key(&key_ref);
            return Err(AppError::Db(e));
        }
    };
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
/// 先读旧状态，keyring 更新失败时回滚数据库修改（codereview P1 原子性）。
pub fn update_provider(
    db: &Db,
    manager: &crate::providers::ProviderManager,
    id: i64,
    name: &str,
    api_url: &str,
    api_key: Option<&str>,
) -> Result<ProviderConfig, AppError> {
    let now = Utc::now().to_rfc3339();
    // 读取旧记录（用于回滚）
    let old: ProviderConfig = db
        .with_conn(|conn| {
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
        })?;
    validate_provider_input(manager, &old.provider_type, api_url)?;

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

    // 更新密钥（可选）；失败则回滚数据库到旧值
    if let Some(key) = api_key {
        if !key.is_empty() {
            if let Err(e) = SecureStorage::update_api_key(&old.key_ref, key) {
                let _ = db.with_conn(|conn| {
                    conn.execute(
                        "UPDATE providers SET name = ?1, api_url = ?2, updated_time = ?3 WHERE id = ?4",
                        rusqlite::params![old.name, old.api_url, now, id],
                    )
                });
                return Err(AppError::Storage(e));
            }
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

/// 删除结果（凭据清理状态可见，P2 修复）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub provider_id: i64,
    pub credential_cleaned: bool,
    pub note: Option<String>,
}

/// 删除 Provider：先删数据库行，再清 keyring 凭证。
/// 凭据清理失败时返回可见状态（不静默），供前端提示用户。
pub fn delete_provider(db: &Db, id: i64) -> Result<DeleteResult, AppError> {
    let key_ref: Option<String> = db.with_conn(|conn| {
        conn.query_row("SELECT key_ref FROM providers WHERE id = ?1", [id], |row| {
            row.get(0)
        })
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

    // P1：仅对真正写入过 keyring 的引用执行清理；空 key_ref（如 Codex CLI 凭证）直接成功
    match key_ref.as_deref() {
        None | Some("") => Ok(DeleteResult {
            provider_id: id,
            credential_cleaned: true,
            note: None,
        }),
        Some(kr) => match SecureStorage::delete_api_key(kr) {
            Ok(()) => Ok(DeleteResult {
                provider_id: id,
                credential_cleaned: true,
                note: None,
            }),
            Err(e) => {
                eprintln!("[delete_provider] 凭据清理失败（key_ref={kr}）: {e}");
                Ok(DeleteResult {
                    provider_id: id,
                    credential_cleaned: false,
                    note: Some(format!(
                        "账户已删除，但系统凭据库中的密钥清理失败，可能残留敏感信息：{e}"
                    )),
                })
            }
        },
    }
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

/// 删除设置项（不存在时静默成功）。
pub fn delete_setting(db: &Db, key: &str) -> Result<(), AppError> {
    db.with_conn(|conn| {
        conn.execute("DELETE FROM settings WHERE key = ?1", [key])?;
        Ok(())
    })
    .map_err(AppError::from)
}

/// 旧凭据迁移结果（供前端提示需重新录入的账户数）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrateResult {
    pub migrated: usize,
    pub failed: usize,
}

/// settings 键名：无法读取的旧凭据数量（启动迁移后写入，供前端提示）。
pub const SETTING_MIGRATION_LEGACY_FAILED: &str = "migration.legacyFailed";

/// 将旧版按名称生成的凭据引用（`provider_<name>`）迁移为 UUID 引用（P1/V3 迁移）。
/// 幂等：仅处理 account 以 `provider_` 开头的旧格式；无法读取的凭据保留记录，
/// 并在结果中统计 failed 供前端提示（不静默丢失）。
pub fn migrate_legacy_credentials(db: &Db) -> Result<MigrateResult, AppError> {
    let rows: Vec<(i64, String)> = db.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT id, key_ref FROM providers")?;
        let iter = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        iter.collect()
    })?;

    let mut migrated = 0;
    let mut failed = 0;
    for (id, key_ref) in rows {
        let account = key_ref.rsplit_once(':').map(|(_, a)| a).unwrap_or("");
        if !account.starts_with("provider_") {
            continue; // 已是 UUID 格式（key_<uuid>）或未知格式
        }
        match SecureStorage::get_api_key(&key_ref) {
            Ok(api_key) => {
                let key_id = SecureStorage::gen_key_id();
                let new_ref = SecureStorage::save_api_key(&key_id, &api_key)?;
                db.with_conn(|conn| {
                    conn.execute(
                        "UPDATE providers SET key_ref = ?1 WHERE id = ?2",
                        rusqlite::params![new_ref, id],
                    )
                })?;
                if let Err(e) = SecureStorage::delete_api_key(&key_ref) {
                    eprintln!("[migrate] 清理旧凭据失败（{key_ref}）: {e}");
                }
                migrated += 1;
            }
            Err(e) => {
                // 凭据无法读取：保留旧记录并统计失败，供前端提示用户重新录入
                eprintln!(
                    "[migrate] 旧凭据无法读取（{key_ref}，provider id={id}）: {e}，请重新录入该账户"
                );
                failed += 1;
            }
        }
    }
    Ok(MigrateResult { migrated, failed })
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
