//! Tauri commands：前端可调用的后端能力（invoke 层）
//!
//! list/add/update/delete_provider、refresh_provider/refresh_all、
//! get/set_refresh_settings、supported_provider_types。

use crate::db::Db;
use crate::providers::{ProviderConfig, ProviderManager, ProviderUsage};
use crate::settings::{self, AppError};
use crate::storage::SecureStorage;
use crate::window_mode::{self, WindowMode, WindowState};
use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, State};

/// 列出全部 Provider。
#[tauri::command]
pub fn list_providers(db: State<'_, Db>) -> Result<Vec<ProviderConfig>, AppError> {
    settings::list_providers(&db)
}

/// 新增 Provider（API Key 走系统凭据库）。
#[tauri::command]
pub fn add_provider(
    db: State<'_, Db>,
    manager: State<'_, ProviderManager>,
    name: String,
    provider_type: String,
    api_url: String,
    api_key: String,
) -> Result<ProviderConfig, AppError> {
    settings::add_provider(&db, &manager, &name, &provider_type, &api_url, &api_key)
}

/// 更新 Provider（api_key 传 Some 才更新密钥）。
#[tauri::command]
pub fn update_provider(
    db: State<'_, Db>,
    manager: State<'_, ProviderManager>,
    id: i64,
    name: String,
    api_url: String,
    api_key: Option<String>,
) -> Result<ProviderConfig, AppError> {
    settings::update_provider(&db, &manager, id, &name, &api_url, api_key.as_deref())
}

/// 删除 Provider（含 keyring 凭据清理）。
#[tauri::command]
pub fn delete_provider(db: State<'_, Db>, id: i64) -> Result<(), AppError> {
    settings::delete_provider(&db, id)
}

/// 支持的 Provider 类型（供前端下拉选择）。
#[tauri::command]
pub fn supported_provider_types(manager: State<'_, ProviderManager>) -> Vec<String> {
    manager.supported_types()
}

/// 立即刷新指定 Provider 并记录当日用量。
#[tauri::command]
pub async fn refresh_provider(
    db: State<'_, Db>,
    manager: State<'_, ProviderManager>,
    id: i64,
) -> Result<ProviderUsage, AppError> {
    let provider: ProviderConfig = settings::list_providers(&db)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or(AppError::ProviderNotFound(id))?;
    let mut usage = fetch_usage(&manager, &provider).await?;
    usage.provider_id = Some(provider.id);
    record_usage(&db, &provider, &usage)?;
    Ok(usage)
}

/// 立即刷新全部启用的 Provider，返回逐账户结果（含失败原因，修复 P1 静默丢弃）。
#[tauri::command]
pub async fn refresh_all(
    db: State<'_, Db>,
    manager: State<'_, ProviderManager>,
) -> Result<Vec<RefreshResult>, AppError> {
    let providers = settings::list_providers(&db)?;
    let mut out = Vec::new();
    for provider in providers.iter().filter(|p| p.enabled) {
        let mut result = RefreshResult {
            provider_id: provider.id,
            provider: provider.provider_type.clone(),
            success: false,
            usage: None,
            error: None,
        };
        match fetch_usage(&manager, provider).await {
            Ok(mut usage) => {
                usage.provider_id = Some(provider.id);
                if let Err(e) = record_usage(&db, provider, &usage) {
                    result.error = Some(format!("记录用量失败: {e}"));
                    out.push(result);
                    continue;
                }
                result.success = true;
                result.usage = Some(usage);
            }
            Err(e) => {
                result.error = Some(e.to_string());
            }
        }
        out.push(result);
    }
    Ok(out)
}

/// 单个 Provider 的刷新结果（成功/失败均返回，前端据此展示可信度）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResult {
    pub provider_id: i64,
    /// provider_type（如 deepseek / openai）
    pub provider: String,
    pub success: bool,
    pub usage: Option<ProviderUsage>,
    pub error: Option<String>,
}

/// 读取刷新策略（前台/后台间隔秒）。
#[tauri::command]
pub fn get_refresh_settings(db: State<'_, Db>) -> Result<RefreshSettings, AppError> {
    Ok(RefreshSettings {
        foreground_secs: settings::refresh_foreground_secs(&db)?,
        background_secs: settings::refresh_background_secs(&db)?,
    })
}

/// 刷新间隔合法性校验（纯函数，供命令与测试复用）。
pub fn validate_refresh_intervals(foreground_secs: u64, background_secs: u64) -> Result<(), String> {
    const MIN_FOREGROUND: u64 = 10;
    const MIN_BACKGROUND: u64 = 60;
    const MAX_INTERVAL: u64 = 3600;

    if !(MIN_FOREGROUND..=MAX_INTERVAL).contains(&foreground_secs) {
        return Err(format!(
            "前台刷新间隔需在 {MIN_FOREGROUND}–{MAX_INTERVAL} 秒之间"
        ));
    }
    if !(MIN_BACKGROUND..=MAX_INTERVAL).contains(&background_secs) {
        return Err(format!(
            "后台刷新间隔需在 {MIN_BACKGROUND}–{MAX_INTERVAL} 秒之间"
        ));
    }
    if background_secs < foreground_secs {
        return Err("后台刷新间隔不能小于前台刷新间隔".into());
    }
    Ok(())
}

/// 写入刷新策略（后端最终校验，修复 P2 数值边界）。
#[tauri::command]
pub fn set_refresh_settings(
    db: State<'_, Db>,
    foreground_secs: u64,
    background_secs: u64,
) -> Result<(), AppError> {
    validate_refresh_intervals(foreground_secs, background_secs)
        .map_err(AppError::Invalid)?;
    settings::set_setting(
        &db,
        settings::SETTING_REFRESH_FOREGROUND_SECS,
        &foreground_secs.to_string(),
    )?;
    settings::set_setting(
        &db,
        settings::SETTING_REFRESH_BACKGROUND_SECS,
        &background_secs.to_string(),
    )?;
    Ok(())
}

/// 切换窗口模式（full / mini / ball）。
#[tauri::command]
pub fn set_window_mode(
    app: AppHandle,
    db: State<'_, Db>,
    mode: WindowMode,
) -> Result<WindowState, AppError> {
    window_mode::apply_mode(&app, &db, mode)?;
    Ok(window_mode::current_state(&db))
}

/// 设置 Always On Top 并持久化。
#[tauri::command]
pub fn set_always_on_top(
    app: AppHandle,
    db: State<'_, Db>,
    enabled: bool,
) -> Result<WindowState, AppError> {
    window_mode::set_always_on_top(&app, &db, enabled)?;
    Ok(window_mode::current_state(&db))
}

/// 读取当前窗口状态（模式 + 置顶）。
#[tauri::command]
pub fn get_window_state(db: State<'_, Db>) -> WindowState {
    window_mode::current_state(&db)
}

/// 刷新策略（返回给前端）。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSettings {
    pub foreground_secs: u64,
    pub background_secs: u64,
}

/// 调用 Provider 适配器获取用量。
async fn fetch_usage(
    manager: &ProviderManager,
    provider: &ProviderConfig,
) -> Result<ProviderUsage, AppError> {
    let adapter = manager
        .get(&provider.provider_type)
        .ok_or_else(|| AppError::Invalid(format!("不支持的 Provider 类型: {}", provider.provider_type)))?;
    let api_key = SecureStorage::get_api_key(&provider.key_ref)?;
    adapter
        .fetch_usage(provider, &api_key)
        .await
        .map_err(|e| AppError::Invalid(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::validate_refresh_intervals;

    #[test]
    fn accepts_valid_intervals() {
        assert!(validate_refresh_intervals(10, 60).is_ok());
        assert!(validate_refresh_intervals(30, 60).is_ok());
        assert!(validate_refresh_intervals(3600, 3600).is_ok());
    }

    #[test]
    fn rejects_out_of_range_values() {
        assert!(validate_refresh_intervals(9, 60).is_err()); // 前台过小
        assert!(validate_refresh_intervals(3601, 3600).is_err()); // 前台过大
        assert!(validate_refresh_intervals(10, 59).is_err()); // 后台过小
        assert!(validate_refresh_intervals(10, 3601).is_err()); // 后台过大
    }

    #[test]
    fn rejects_background_smaller_than_foreground() {
        assert!(validate_refresh_intervals(60, 30).is_err());
        assert!(validate_refresh_intervals(300, 60).is_err());
    }
}

/// 将一次用量写入 usage_history（按 provider+date UPSERT，供日报/周报/月报）。
fn record_usage(db: &Db, provider: &ProviderConfig, usage: &ProviderUsage) -> Result<(), AppError> {
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let raw = serde_json::to_string(usage).unwrap_or_default();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO usage_history (provider_id, date, tokens, cost, balance, raw_json, created_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(provider_id, date) DO UPDATE SET
               tokens = excluded.tokens,
               cost = excluded.cost,
               balance = excluded.balance,
               raw_json = excluded.raw_json",
            rusqlite::params![
                provider.id,
                date,
                usage.total_tokens as i64,
                usage.today_cost.unwrap_or(0.0),
                usage.balance,
                raw,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    })
    .map_err(AppError::from)
}
