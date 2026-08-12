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
use std::sync::Arc;
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
    manager: State<'_, Arc<ProviderManager>>,
    name: String,
    provider_type: String,
    api_url: String,
    api_key: String,
) -> Result<ProviderConfig, AppError> {
    settings::add_provider(&db, manager.inner().as_ref(), &name, &provider_type, &api_url, &api_key)
}

/// 更新 Provider（api_key 传 Some 才更新密钥）。
#[tauri::command]
pub fn update_provider(
    db: State<'_, Db>,
    manager: State<'_, Arc<ProviderManager>>,
    id: i64,
    name: String,
    api_url: String,
    api_key: Option<String>,
) -> Result<ProviderConfig, AppError> {
    settings::update_provider(&db, manager.inner().as_ref(), id, &name, &api_url, api_key.as_deref())
}

/// 删除 Provider（含 keyring 凭据清理，清理失败返回可见状态）。
#[tauri::command]
pub fn delete_provider(db: State<'_, Db>, id: i64) -> Result<settings::DeleteResult, AppError> {
    settings::delete_provider(&db, id)
}

/// 支持的 Provider 类型（供前端下拉选择）。
#[tauri::command]
pub fn supported_provider_types(manager: State<'_, Arc<ProviderManager>>) -> Vec<String> {
    manager.supported_types()
}

/// 立即刷新指定 Provider 并记录当日用量。
#[tauri::command]
pub async fn refresh_provider(
    db: State<'_, Db>,
    manager: State<'_, Arc<ProviderManager>>,
    id: i64,
) -> Result<ProviderUsage, AppError> {
    let provider: ProviderConfig = settings::list_providers(&db)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or(AppError::ProviderNotFound(id))?;
    let mut usage = fetch_usage(manager.inner().as_ref(), &provider).await?;
    usage.provider_id = Some(provider.id);
    record_usage(&db, &provider, &usage)?;
    Ok(usage)
}

/// 立即刷新全部启用的 Provider，返回逐账户结果（含失败原因，修复 P1 静默丢弃）。
#[tauri::command]
pub async fn refresh_all(
    db: State<'_, Db>,
    manager: State<'_, Arc<ProviderManager>>,
) -> Result<Vec<RefreshResult>, AppError> {
    use std::sync::Arc as StdArc;
    // P2：受控并发（最多 3 个同时请求），避免慢账户串行阻塞全部结果
    const MAX_CONCURRENCY: usize = 3;

    let providers = settings::list_providers(&db)?;
    let enabled: Vec<&ProviderConfig> = providers.iter().filter(|p| p.enabled).collect();
    if enabled.is_empty() {
        return Ok(Vec::new());
    }

    // 并发执行 fetch（record_usage 需要独占 db 锁，放在结果收集阶段顺序执行）
    let sem = StdArc::new(tokio::sync::Semaphore::new(MAX_CONCURRENCY));
    let mut tasks = tokio::task::JoinSet::new();
    for p in enabled {
        let permit = sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::Invalid(format!("刷新并发控制失败: {e}")))?;
        let manager_arc = manager.inner().clone();
        let provider = p.clone();
        tasks.spawn(async move {
            let _permit = permit;
            let usage = fetch_usage(manager_arc.as_ref(), &provider).await;
            (provider, usage)
        });
    }
    let mut fetched = Vec::new();
    while let Some(res) = tasks.join_next().await {
        let (provider, usage) = res
            .map_err(|e| AppError::Invalid(format!("刷新任务失败: {e}")))?;
        fetched.push((provider, usage));
    }

    let mut out = Vec::new();
    for (provider, res) in fetched {
        let mut result = RefreshResult {
            provider_id: provider.id,
            provider: provider.provider_type.clone(),
            success: false,
            usage: None,
            error: None,
        };
        match res {
            Ok(mut usage) => {
                usage.provider_id = Some(provider.id);
                if let Err(e) = record_usage(&db, &provider, &usage) {
                    result.error = Some(format!("记录用量失败: {e}"));
                } else {
                    result.success = true;
                    result.usage = Some(usage);
                }
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

/// 读取旧凭据迁移失败数（供前端提示需重新录入的账户）。
#[tauri::command]
pub fn get_migration_status(db: State<'_, Db>) -> Option<u64> {
    settings::get_setting(&db, settings::SETTING_MIGRATION_LEGACY_FAILED)
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
}

/// 读取 DIY 布局 JSON（V0.3；未设置时返回 None）。
#[tauri::command]
pub fn get_layout(db: State<'_, Db>) -> Result<Option<String>, AppError> {
    settings::get_setting(&db, settings::SETTING_LAYOUT)
}

/// 布局 JSON 校验（纯函数，供命令与测试复用，V0.3）。
/// P2：校验 theme、widgets 数组、每项 id/type/visible、ID 唯一性、数量与大小限制。
pub fn validate_layout_json(layout: &str) -> Result<(), String> {
    const MAX_JSON_BYTES: usize = 64 * 1024;
    const MAX_WIDGETS: usize = 20;
    if layout.len() > MAX_JSON_BYTES {
        return Err("布局 JSON 过大".into());
    }
    let value: serde_json::Value =
        serde_json::from_str(layout).map_err(|e| format!("布局 JSON 无效: {e}"))?;
    let theme = value.get("theme").and_then(|t| t.as_str()).unwrap_or("dark");
    if theme != "dark" && theme != "light" {
        return Err("theme 仅支持 dark / light".into());
    }
    let Some(arr) = value.get("widgets").and_then(|w| w.as_array()) else {
        return Err("widgets 必须为数组".into());
    };
    if arr.len() > MAX_WIDGETS {
        return Err(format!("Widget 数量超限（最多 {MAX_WIDGETS} 个）"));
    }
    let mut ids = std::collections::HashSet::new();
    for w in arr {
        let id = w.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let ty = w.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let visible = w.get("visible");
        if id.is_empty() {
            return Err("Widget id 不能为空".into());
        }
        if !ids.insert(id) {
            return Err(format!("Widget id 重复: {id}"));
        }
        if !["providers", "summary", "cost"].contains(&ty) {
            return Err(format!("未知 Widget 类型: {ty}"));
        }
        if !visible.is_some_and(|v| v.is_boolean()) {
            return Err("visible 必须为布尔值".into());
        }
    }
    Ok(())
}

/// 保存 DIY 布局 JSON（V0.3）。后端做最小结构校验（可解析、theme 合法、widgets 为数组）。
#[tauri::command]
pub fn set_layout(db: State<'_, Db>, layout: String) -> Result<(), AppError> {
    validate_layout_json(&layout).map_err(AppError::Invalid)?;
    settings::set_setting(&db, settings::SETTING_LAYOUT, &layout)
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
    // 凭据来源判断：Codex 复用 CLI 本地凭证（~/.codex/auth.json），不走 keyring
    let api_key = if provider.credential_source() == crate::providers::CredentialSource::CodexCli {
        String::new()
    } else {
        SecureStorage::get_api_key(&provider.key_ref)?
    };
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

    #[test]
    fn layout_validation_accepts_valid_json() {
        let ok = r#"{"theme":"dark","widgets":[{"id":"w1","type":"providers","visible":true}]}"#;
        assert!(super::validate_layout_json(ok).is_ok());
        let ok_light = r#"{"theme":"light","widgets":[]}"#;
        assert!(super::validate_layout_json(ok_light).is_ok());
    }

    #[test]
    fn layout_validation_rejects_bad_input() {
        assert!(super::validate_layout_json("not json").is_err());
        assert!(super::validate_layout_json(r#"{"theme":"blue","widgets":[]}"#).is_err());
        assert!(super::validate_layout_json(r#"{"theme":"dark","widgets":{}}"#).is_err());
    }

    #[test]
    fn layout_validation_rejects_duplicate_id_and_unknown_type() {
        let dup = r#"{"theme":"dark","widgets":[
            {"id":"w1","type":"providers","visible":true},
            {"id":"w1","type":"summary","visible":true}
        ]}"#;
        assert!(super::validate_layout_json(dup).is_err());
        let unknown = r#"{"theme":"dark","widgets":[
            {"id":"w1","type":"chart","visible":true}
        ]}"#;
        assert!(super::validate_layout_json(unknown).is_err());
        let missing_visible = r#"{"theme":"dark","widgets":[
            {"id":"w1","type":"providers"}
        ]}"#;
        assert!(super::validate_layout_json(missing_visible).is_err());
    }

    #[test]
    fn layout_validation_rejects_oversize_and_too_many_widgets() {
        let big = format!(r#"{{"theme":"dark","widgets":{}"#, vec!["[]"; 1].join(","));
        let too_many = format!(
            r#"{{"theme":"dark","widgets":[{}]}}"#,
            (0..21)
                .map(|i| format!(r#"{{"id":"w{i}","type":"summary","visible":true}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(super::validate_layout_json(&too_many).is_err());
        // 超长 JSON
        let long = format!(
            r#"{{"theme":"dark","widgets":[{}]}}"#,
            (0..6000)
                .map(|i| format!(r#"{{"id":"w{i}","type":"summary","visible":true}}"#))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(super::validate_layout_json(&long).is_err());
        let _ = big;
    }
}

#[cfg(test)]
mod codex_url_tests {
    use crate::providers::ProviderManager;
    use crate::settings::validate_provider_input;

    #[test]
    fn codex_accepts_exact_official_url() {
        let m = ProviderManager::new();
        assert!(validate_provider_input(&m, "codex", "https://chatgpt.com/backend-api/codex").is_ok());
        assert!(validate_provider_input(&m, "codex", "https://chatgpt.com/backend-api/codex/").is_ok());
    }

    #[test]
    fn codex_rejects_any_other_url() {
        let m = ProviderManager::new();
        // 恶意 host
        assert!(validate_provider_input(&m, "codex", "https://evil.example.com").is_err());
        // 子域伪装（chatgpt.com.evil.test）
        assert!(validate_provider_input(&m, "codex", "https://chatgpt.com.evil.test/backend-api/codex").is_err());
        // 非默认端口
        assert!(validate_provider_input(&m, "codex", "https://chatgpt.com:8443/backend-api/codex").is_err());
        // userinfo
        assert!(validate_provider_input(&m, "codex", "https://user:pass@chatgpt.com/backend-api/codex").is_err());
        // 路径混淆
        assert!(validate_provider_input(&m, "codex", "https://chatgpt.com/backend-api/codex/../codex").is_err());
        // 非 https
        assert!(validate_provider_input(&m, "codex", "http://chatgpt.com/backend-api/codex").is_err());
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
