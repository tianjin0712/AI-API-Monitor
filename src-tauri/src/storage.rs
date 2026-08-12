//! Secure Storage：API Key 安全存储（keyring 封装）
//!
//! Windows → Credential Manager，macOS → Keychain，Linux → Secret Service。
//! API Key 绝不落 SQLite，数据库仅存 keyring 引用（key_ref = "service:account"）。
//!
//! 凭据 account 使用不可预测的 uuid（key_id）而非展示名称，
//! 避免同名账户互相覆盖（codereview P0）。

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
    /// 生成一个新的凭据标识（uuid v4）。
    pub fn gen_key_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// 保存 API Key，返回 keyring 引用（格式 `service:key_<key_id>`）。
    pub fn save_api_key(key_id: &str, api_key: &str) -> Result<String, StorageError> {
        let account = account_for(key_id);
        let entry = Entry::new(KEYRING_SERVICE, &account)?;
        entry.set_password(api_key)?;
        Ok(format!("{KEYRING_SERVICE}:{account}"))
    }

    /// 读取 API Key（按 key_ref）。
    pub fn get_api_key(key_ref: &str) -> Result<String, StorageError> {
        let (_service, account) = parse_ref(key_ref)?;
        let entry = Entry::new(KEYRING_SERVICE, account)?;
        Ok(entry.get_password()?)
    }

    /// 删除 API Key（按 key_ref）。
    pub fn delete_api_key(key_ref: &str) -> Result<(), StorageError> {
        let (_service, account) = parse_ref(key_ref)?;
        let entry = Entry::new(KEYRING_SERVICE, account)?;
        entry.delete_credential()?;
        Ok(())
    }

    /// 更新 API Key（key_ref 不变）。
    pub fn update_api_key(key_ref: &str, api_key: &str) -> Result<(), StorageError> {
        let (_service, account) = parse_ref(key_ref)?;
        let entry = Entry::new(KEYRING_SERVICE, account)?;
        entry.set_password(api_key)?;
        Ok(())
    }
}

/// 由 key_id 生成稳定的 account 标识（`key_<uuid>`，uuid 保证唯一）。
fn account_for(key_id: &str) -> String {
    format!("key_{}", key_id.trim())
}

/// 解析 key_ref（`service:account`）为二元组。
fn parse_ref(key_ref: &str) -> Result<(&str, &str), StorageError> {
    key_ref
        .split_once(':')
        .filter(|(s, a)| !s.is_empty() && !a.is_empty())
        .ok_or_else(|| StorageError::InvalidRef(key_ref.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_key_ids_produce_distinct_accounts() {
        let a = account_for("key_11111111-1111-1111-1111-111111111111");
        let b = account_for("key_22222222-2222-2222-2222-222222222222");
        assert_ne!(a, b, "不同 key_id 必须映射到不同 account（修复同名覆盖）");
    }

    #[test]
    fn same_key_id_is_stable() {
        let id = "key_abc";
        assert_eq!(account_for(id), account_for(id));
    }

    #[test]
    fn parse_ref_roundtrip() {
        let (s, a) = parse_ref("com.aiapimonitor.desktop:key_abc").expect("parse ok");
        assert_eq!(s, "com.aiapimonitor.desktop");
        assert_eq!(a, "key_abc");
    }

    #[test]
    fn parse_ref_rejects_malformed() {
        assert!(parse_ref("no-colon").is_err());
        assert!(parse_ref("service:").is_err());
        assert!(parse_ref(":account").is_err());
    }
}
