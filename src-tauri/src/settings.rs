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
pub const SETTING_TELEMETRY_ENABLED: &str = "privacy.telemetryEnabled";
pub const SETTING_APPROVED_CUSTOM_ORIGINS: &str = "network.approvedCustomOrigins";
pub const SETTING_CLOSE_BEHAVIOR: &str = "app.closeBehavior";

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
        serializer.serialize_str(&crate::security::SensitiveDataFilter::redact(
            &self.to_string(),
        ))
    }
}

/// 校验 Provider 输入：类型白名单 + URL 合法性（修复 P2）。
/// 所有携带凭据的 Provider 请求都必须使用 HTTPS。
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
    let official_url = match provider_type {
        "deepseek" => Some("https://api.deepseek.com"),
        "openai" => Some("https://api.openai.com/v1"),
        "openrouter" => Some("https://openrouter.ai"),
        "siliconflow" => Some("https://api.siliconflow.cn/v1"),
        "claude" => Some("https://api.anthropic.com/v1"),
        "custom" => None,
        _ => None,
    };
    if let Some(official_url) = official_url {
        if api_url.trim_end_matches('/') != official_url {
            return Err(AppError::Invalid(
                "内置 Provider 必须使用注册表中的固定官方 HTTPS 地址；自定义网关请使用 custom 类型"
                    .into(),
            ));
        }
        return Ok(());
    }
    let parsed =
        url::Url::parse(api_url).map_err(|_| AppError::Invalid("API URL 格式无效".into()))?;
    if parsed.scheme() != "https" {
        return Err(AppError::Invalid("API 地址必须使用 https://".into()));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::Invalid("API 地址禁止包含用户名或密码".into()));
    }
    if parsed.host_str().is_none() {
        return Err(AppError::Invalid("API 地址缺少有效主机名".into()));
    }
    Ok(())
}

/// 列出全部 Provider（不含 API Key）。
pub fn list_providers(db: &Db) -> Result<Vec<ProviderConfig>, AppError> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, provider_type, api_url, key_ref, key_hint, enabled, created_time, updated_time
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
    if provider_type == "custom" && !is_custom_endpoint_approved(db, api_url)? {
        return Err(AppError::Invalid(
            "自定义网关尚未获得用户明确批准，请确认目标域名后重试".into(),
        ));
    }
    // Codex 仅查询 CLI 公开登录状态，不读取认证文件，也不写入 keyring。
    let key_ref = if api_key.is_empty() {
        String::new()
    } else {
        let key_id = SecureStorage::gen_key_id();
        SecureStorage::save_api_key(&key_id, api_key)?
    };
    let key_hint = crate::security::mask_secret(api_key);
    let now = Utc::now().to_rfc3339();
    let insert = db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO providers (name, provider_type, api_url, key_ref, key_hint, enabled, created_time, updated_time)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
            rusqlite::params![name, provider_type, api_url, key_ref, key_hint, now],
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
        key_hint,
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
            "SELECT id, name, provider_type, api_url, key_ref, key_hint, enabled, created_time, updated_time
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
    if old.provider_type == "custom" && !is_custom_endpoint_approved(db, api_url)? {
        return Err(AppError::Invalid(
            "自定义网关尚未获得用户明确批准，请确认目标域名后重试".into(),
        ));
    }

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
            let key_hint = crate::security::mask_secret(key);
            db.with_conn(|conn| {
                conn.execute(
                    "UPDATE providers SET key_hint = ?1 WHERE id = ?2",
                    rusqlite::params![key_hint, id],
                )?;
                Ok(())
            })?;
        }
    }
    // 返回更新后的完整记录
    db.with_conn(|conn| {
        conn.query_row(
            "SELECT id, name, provider_type, api_url, key_ref, key_hint, enabled, created_time, updated_time
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
                crate::security::safe_log("delete_provider", format!("凭据清理失败: {e}"));
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

/// Store a sensitive local field with AES-256-GCM. The encryption key is kept
/// only in the operating-system keyring and never in SQLite/config files.
pub fn set_sensitive_setting(db: &Db, key: &str, value: &str) -> Result<(), AppError> {
    let master_key = SecureStorage::data_encryption_key()?;
    let (ciphertext, nonce) = crate::security::encrypt_sensitive(&master_key, value.as_bytes())
        .map_err(|error| AppError::Invalid(error.to_string()))?;
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO secure_settings (key, nonce, ciphertext) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET nonce = excluded.nonce, ciphertext = excluded.ciphertext",
            rusqlite::params![key, nonce.as_slice(), ciphertext],
        )?;
        Ok(())
    })?;
    Ok(())
}

#[allow(dead_code)]
pub fn get_sensitive_setting(
    db: &Db,
    key: &str,
) -> Result<Option<zeroize::Zeroizing<String>>, AppError> {
    let row: Option<(Vec<u8>, Vec<u8>)> = db.with_conn(|conn| {
        conn.query_row(
            "SELECT nonce, ciphertext FROM secure_settings WHERE key = ?1",
            [key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
    })?;
    let Some((nonce, ciphertext)) = row else {
        return Ok(None);
    };
    let nonce: [u8; 12] = nonce
        .try_into()
        .map_err(|_| AppError::Invalid("敏感字段 nonce 无效".into()))?;
    let master_key = SecureStorage::data_encryption_key()?;
    let clear = crate::security::decrypt_sensitive(&master_key, &nonce, &ciphertext)
        .map_err(|error| AppError::Invalid(error.to_string()))?;
    let value = String::from_utf8(clear.to_vec())
        .map_err(|_| AppError::Invalid("敏感字段编码无效".into()))?;
    Ok(Some(zeroize::Zeroizing::new(value)))
}

fn is_sensitive_setting_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "token",
        "cookie",
        "password",
        "secret",
        "authorization",
    ]
    .iter()
    .any(|marker| key.contains(marker))
}

/// Migrate any legacy plaintext sensitive setting into the encrypted table.
pub fn migrate_sensitive_settings(db: &Db) -> Result<usize, AppError> {
    let rows: Vec<(String, String)> = db.with_conn(|conn| {
        let mut statement = conn.prepare("SELECT key, value FROM settings")?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect();
        rows
    })?;
    let mut migrated = 0;
    for (key, value) in rows {
        if is_sensitive_setting_key(&key) {
            set_sensitive_setting(db, &key, &value)?;
            delete_setting(db, &key)?;
            migrated += 1;
        }
    }
    Ok(migrated)
}

pub fn ensure_privacy_defaults(db: &Db) -> Result<(), AppError> {
    if get_setting(db, SETTING_TELEMETRY_ENABLED)?.is_none() {
        set_setting(db, SETTING_TELEMETRY_ENABLED, "false")?;
    }
    Ok(())
}

fn approved_custom_origins(db: &Db) -> Result<Vec<String>, AppError> {
    let Some(json) = get_setting(db, SETTING_APPROVED_CUSTOM_ORIGINS)? else {
        return Ok(Vec::new());
    };
    let values: Vec<String> = serde_json::from_str(&json).unwrap_or_default();
    Ok(values
        .into_iter()
        .filter(|origin| crate::security::custom_endpoint_origin(origin).is_ok())
        .take(32)
        .collect())
}

pub fn is_custom_endpoint_approved(db: &Db, api_url: &str) -> Result<bool, AppError> {
    let origin = crate::security::custom_endpoint_origin(api_url).map_err(AppError::Invalid)?;
    Ok(approved_custom_origins(db)?
        .iter()
        .any(|approved| approved == &origin))
}

pub fn approve_custom_endpoint(db: &Db, api_url: &str) -> Result<String, AppError> {
    let origin = crate::security::custom_endpoint_origin(api_url).map_err(AppError::Invalid)?;
    let mut approved = approved_custom_origins(db)?;
    if !approved.iter().any(|value| value == &origin) {
        if approved.len() >= 32 {
            return Err(AppError::Invalid("已批准的自定义网关数量达到上限".into()));
        }
        approved.push(origin.clone());
        approved.sort();
        set_setting(
            db,
            SETTING_APPROVED_CUSTOM_ORIGINS,
            &serde_json::to_string(&approved)
                .map_err(|_| AppError::Invalid("无法保存自定义网关批准记录".into()))?,
        )?;
    }
    Ok(origin)
}

/// Populate non-sensitive masked hints for records created before schema V5.
pub fn migrate_missing_key_hints(db: &Db) -> Result<usize, AppError> {
    let rows: Vec<(i64, String)> = db.with_conn(|conn| {
        let mut statement = conn
            .prepare("SELECT id, key_ref FROM providers WHERE key_ref <> '' AND key_hint = ''")?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect();
        rows
    })?;
    let mut migrated = 0;
    for (id, key_ref) in rows {
        match SecureStorage::get_api_key(&key_ref) {
            Ok(key) => {
                let hint = crate::security::mask_secret(&key);
                db.with_conn(|conn| {
                    conn.execute(
                        "UPDATE providers SET key_hint = ?1 WHERE id = ?2",
                        rusqlite::params![hint, id],
                    )?;
                    Ok(())
                })?;
                migrated += 1;
            }
            Err(error) => crate::security::safe_log(
                "migration",
                format!("provider id={id} 的 Key 掩码迁移失败: {error}"),
            ),
        }
    }
    Ok(migrated)
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
                        "UPDATE providers SET key_ref = ?1, key_hint = ?2 WHERE id = ?3",
                        rusqlite::params![new_ref, crate::security::mask_secret(&api_key), id],
                    )
                })?;
                if let Err(e) = SecureStorage::delete_api_key(&key_ref) {
                    crate::security::safe_log("migrate", format!("清理旧凭据失败: {e}"));
                }
                migrated += 1;
            }
            Err(e) => {
                // 凭据无法读取：保留旧记录并统计失败，供前端提示用户重新录入
                crate::security::safe_log(
                    "migrate",
                    format!("旧凭据无法读取（provider id={id}）: {e}，请重新录入该账户"),
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
        key_hint: row.get(5)?,
        enabled: row.get::<_, i64>(6)? != 0,
        created_time: row.get(7)?,
        updated_time: row.get(8)?,
    })
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn rejects_all_http_provider_urls() {
        let manager = crate::providers::ProviderManager::new();
        for url in [
            "http://example.com",
            "http://localhost:8080",
            "file:///tmp/key",
        ] {
            assert!(validate_provider_input(&manager, "deepseek", url).is_err());
        }
        assert!(validate_provider_input(&manager, "deepseek", "https://api.deepseek.com").is_ok());
        assert!(validate_provider_input(&manager, "deepseek", "https://evil.example").is_err());
        assert!(validate_provider_input(&manager, "custom", "https://gateway.example/v1").is_ok());
    }

    #[test]
    fn identifies_sensitive_setting_names() {
        for key in [
            "provider.api_key",
            "oauth.refreshToken",
            "session.cookie",
            "db.password",
        ] {
            assert!(is_sensitive_setting_key(key));
        }
        assert!(!is_sensitive_setting_key(SETTING_LAYOUT));
    }

    #[test]
    fn custom_endpoint_approval_is_explicit_and_origin_scoped() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);\n\
                 CREATE TABLE secure_settings (key TEXT PRIMARY KEY, ciphertext BLOB NOT NULL, nonce BLOB NOT NULL);",
            )
            .unwrap();
        let db = Db(std::sync::Mutex::new(connection));
        let endpoint = "https://gateway.example.com/v1";
        assert!(!is_custom_endpoint_approved(&db, endpoint).unwrap());
        assert_eq!(
            approve_custom_endpoint(&db, endpoint).unwrap(),
            "https://gateway.example.com"
        );
        assert!(is_custom_endpoint_approved(&db, endpoint).unwrap());
        assert!(is_custom_endpoint_approved(&db, "https://gateway.example.com/v2").unwrap());
        assert!(!is_custom_endpoint_approved(&db, "https://other.example.com/v1").unwrap());
    }
}
