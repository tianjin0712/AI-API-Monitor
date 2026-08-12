//! Secure Storage：API Key 安全存储（keyring 封装）
//!
//! Windows → Credential Manager，macOS → Keychain，Linux → Secret Service。
//! API Key 绝不落 SQLite，数据库仅存 keyring 引用（key_ref = "service:account"）。

use keyring::Entry;

/// keyring service 名（与 tauri.conf.json identifier 对齐）。
const KEYRING_SERVICE: &str = "com.aiapimonitor.desktop";

/// 安全存储错误。
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("密钥存储失败: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("无效的 key_ref 格式: {0}")]
    InvalidRef(String),
}

/// API Key 安全存储。
pub struct SecureStorage;

impl SecureStorage {
    /// 保存 API Key，返回 keyring 引用（格式 `service:account`）。
    pub fn save_api_key(provider_name: &str, api_key: &str) -> Result<String, StorageError> {
        let account = account_for(provider_name);
        let entry = Entry::new(KEYRING_SERVICE, &account)?;
        entry.set_password(api_key)?;
        Ok(format!("{KEYRING_SERVICE}:{account}"))
    }

    /// 读取 API Key（按 key_ref）。
    pub fn get_api_key(key_ref: &str) -> Result<String, StorageError> {
        let (_service, account) = parse_ref(key_ref)?;
        let entry = Entry::new(KEYRING_SERVICE, &account)?;
        Ok(entry.get_password()?)
    }

    /// 删除 API Key（按 key_ref）。
    pub fn delete_api_key(key_ref: &str) -> Result<(), StorageError> {
        let (_service, account) = parse_ref(key_ref)?;
        let entry = Entry::new(KEYRING_SERVICE, &account)?;
        entry.delete_credential()?;
        Ok(())
    }

    /// 更新 API Key（key_ref 不变）。
    pub fn update_api_key(key_ref: &str, api_key: &str) -> Result<(), StorageError> {
        let (_service, account) = parse_ref(key_ref)?;
        let entry = Entry::new(KEYRING_SERVICE, &account)?;
        entry.set_password(api_key)?;
        Ok(())
    }
}

/// 由 provider 名生成稳定的 account 标识。
/// 避开 keyring 对 service/account 中冒号等字符的限制。
fn account_for(provider_name: &str) -> String {
    let sanitized: String = provider_name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    format!("provider_{}", sanitized)
}

/// 解析 key_ref（`service:account`）为二元组。
fn parse_ref(key_ref: &str) -> Result<(&str, &str), StorageError> {
    key_ref
        .split_once(':')
        .filter(|(s, a)| !s.is_empty() && !a.is_empty())
        .ok_or_else(|| StorageError::InvalidRef(key_ref.to_string()))
}
