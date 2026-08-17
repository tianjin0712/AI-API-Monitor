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
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

/// Import image bytes into the isolated application asset directory.
#[tauri::command]
pub fn import_asset(
    assets: State<'_, crate::assets::AssetStore>,
    original_name: String,
    data: Vec<u8>,
) -> Result<crate::assets::ImportedAsset, AppError> {
    assets
        .import(&original_name, &data)
        .map_err(|error| AppError::Invalid(error.to_string()))
}

/// Delete only an opaque asset previously created by this application.
#[tauri::command]
pub fn delete_asset(
    assets: State<'_, crate::assets::AssetStore>,
    asset_id: String,
) -> Result<(), AppError> {
    assets
        .delete(&asset_id)
        .map_err(|error| AppError::Invalid(error.to_string()))
}

/// Read an imported image back for local color analysis.
#[tauri::command]
pub fn read_asset(
    assets: State<'_, crate::assets::AssetStore>,
    asset_id: String,
) -> Result<Vec<u8>, AppError> {
    assets
        .read_bytes(&asset_id)
        .map_err(|error| AppError::Invalid(error.to_string()))
}

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
    let api_key = zeroize::Zeroizing::new(api_key);
    settings::add_provider(
        &db,
        manager.inner().as_ref(),
        &name,
        &provider_type,
        &api_url,
        api_key.as_str(),
    )
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
    let api_key = api_key.map(zeroize::Zeroizing::new);
    settings::update_provider(
        &db,
        manager.inner().as_ref(),
        id,
        &name,
        &api_url,
        api_key.as_ref().map(|value| value.as_str()),
    )
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

#[tauri::command]
pub fn is_custom_endpoint_approved(db: State<'_, Db>, api_url: String) -> Result<bool, AppError> {
    settings::is_custom_endpoint_approved(&db, &api_url)
}

#[tauri::command]
pub fn approve_custom_endpoint(db: State<'_, Db>, api_url: String) -> Result<String, AppError> {
    settings::approve_custom_endpoint(&db, &api_url)
}

/// 立即刷新指定 Provider 并记录当日用量。
#[tauri::command]
pub async fn refresh_provider(
    app: AppHandle,
    db: State<'_, Db>,
    manager: State<'_, Arc<ProviderManager>>,
    alerts: State<'_, AlertState>,
    id: i64,
) -> Result<ProviderUsage, AppError> {
    let provider: ProviderConfig = settings::list_providers(&db)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or(AppError::ProviderNotFound(id))?;
    let mut usage = fetch_usage(manager.inner().as_ref(), &provider).await?;
    usage.provider_id = Some(provider.id);
    record_usage(&db, &provider, &usage)?;
    let days_left = predict_for(&db, provider.id, 7)
        .ok()
        .flatten()
        .and_then(|p| p.days_left);
    check_alerts(&app, &alerts, &provider, &usage, days_left);
    Ok(usage)
}

/// 立即刷新全部启用的 Provider，返回逐账户结果（含失败原因，修复 P1 静默丢弃）。
#[tauri::command]
pub async fn refresh_all(
    app: AppHandle,
    db: State<'_, Db>,
    manager: State<'_, Arc<ProviderManager>>,
    alerts: State<'_, AlertState>,
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
        let (provider, usage) = res.map_err(|e| AppError::Invalid(format!("刷新任务失败: {e}")))?;
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
                    result.usage = Some(usage.clone());
                    let days_left = predict_for(&db, provider.id, 7)
                        .ok()
                        .flatten()
                        .and_then(|p| p.days_left);
                    check_alerts(&app, &alerts, &provider, &usage, days_left);
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

#[tauri::command]
pub async fn get_codex_runtime_status(
) -> Result<crate::providers::codex::CodexRuntimeStatus, AppError> {
    tokio::task::spawn_blocking(crate::providers::codex::runtime_status)
        .await
        .map_err(|_| AppError::Invalid("Codex Runtime 状态检测失败".into()))
}

#[tauri::command]
pub async fn start_codex_login() -> Result<(), AppError> {
    tokio::task::spawn_blocking(crate::providers::codex::start_login)
        .await
        .map_err(|_| AppError::Invalid("Codex 登录任务启动失败".into()))?
        .map_err(|error| AppError::Invalid(error.to_string()))
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

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppBehaviorSettings {
    pub close_behavior: String,
    pub auto_start: bool,
}

#[tauri::command]
pub fn get_app_behavior_settings(
    app: AppHandle,
    db: State<'_, Db>,
) -> Result<AppBehaviorSettings, AppError> {
    let close_behavior = settings::get_setting(&db, settings::SETTING_CLOSE_BEHAVIOR)?
        .filter(|value| value == "minimize_to_tray" || value == "quit")
        .unwrap_or_else(|| "minimize_to_tray".to_string());
    let auto_start = app
        .autolaunch()
        .is_enabled()
        .map_err(|e| AppError::Invalid(e.to_string()))?;
    Ok(AppBehaviorSettings {
        close_behavior,
        auto_start,
    })
}

#[tauri::command]
pub fn set_close_behavior(db: State<'_, Db>, close_behavior: String) -> Result<(), AppError> {
    if close_behavior != "minimize_to_tray" && close_behavior != "quit" {
        return Err(AppError::Invalid("关闭按钮行为无效".into()));
    }
    settings::set_setting(&db, settings::SETTING_CLOSE_BEHAVIOR, &close_behavior)
}

#[tauri::command]
pub fn set_auto_start(app: AppHandle, enabled: bool) -> Result<bool, AppError> {
    let result = if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    };
    result.map_err(|_| {
        AppError::Invalid(format!(
            "无法{}开机自启动，请检查系统权限",
            if enabled { "启用" } else { "关闭" }
        ))
    })?;
    app.autolaunch()
        .is_enabled()
        .map_err(|e| AppError::Invalid(e.to_string()))
}

/// 读取旧凭据迁移失败数（供前端提示需重新录入的账户）。
#[tauri::command]
pub fn get_migration_status(db: State<'_, Db>) -> Option<u64> {
    settings::get_setting(&db, settings::SETTING_MIGRATION_LEGACY_FAILED)
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
}

/// Read and clear the one-time database recovery notice shown after startup.
#[tauri::command]
pub fn get_database_recovery_notice(db: State<'_, Db>) -> Option<String> {
    let notice = settings::get_setting(&db, settings::SETTING_DATABASE_RECOVERY_NOTICE)
        .ok()
        .flatten();
    if notice.is_some() {
        let _ = settings::delete_setting(&db, settings::SETTING_DATABASE_RECOVERY_NOTICE);
    }
    notice
}

// ---- V0.5 高级统计 ----

/// 单日用量（历史序列数据点）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub date: String, // YYYY-MM-DD
    /// 累计 Token 快照（兼容历史；非当日趋势指标）
    pub tokens: u64,
    /// 当日 Token（NULL=平台不提供/未知）
    pub today_tokens: Option<i64>,
    /// 当日费用（NULL=平台不提供/未知，不伪装成 0）
    pub cost: Option<f64>,
    pub balance: Option<f64>,
}

/// 历史窗口偏移（纯函数，V0.5 复审 P1）：包含今天的 N 天窗口 = -(days-1) 天。
pub fn history_start_offset(days: u64) -> String {
    format!("-{} days", days.saturating_sub(1))
}

/// 查询历史用量序列（按 provider，可空=全部；days 默认 30，上限 365）。
#[tauri::command]
pub fn get_usage_history(
    db: State<'_, Db>,
    provider_id: Option<i64>,
    days: Option<u64>,
) -> Result<Vec<DailyUsage>, AppError> {
    let days = days.unwrap_or(30).clamp(1, 365);
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT date, tokens, today_tokens, cost, balance FROM usage_history
             WHERE (?1 IS NULL OR provider_id = ?1)
               AND date >= date('now', ?2)
             ORDER BY date",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![provider_id, history_start_offset(days)],
            |row| {
                Ok(DailyUsage {
                    date: row.get(0)?,
                    tokens: row.get::<_, i64>(1)? as u64,
                    today_tokens: row.get(2)?,
                    cost: row.get(3)?,
                    balance: row.get(4)?,
                })
            },
        )?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    })
    .map_err(AppError::from)
}

/// 消耗预测：近 N 天有效费用样本日均 + 当前余额 → 预计剩余天数与耗尽日期。
/// V0.5 复审 P1：只使用真实费用样本（NULL 视为未知），按"有数据日"平均；补样本数/覆盖天数。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prediction {
    pub daily_cost_avg: f64,
    /// 参与平均的有效费用样本数
    pub samples: usize,
    /// 覆盖的天数跨度（今天往前含 N 天）
    pub days_span: u64,
    pub balance: Option<f64>,
    pub days_left: Option<f64>,
    pub exhausted_date: Option<String>,
}

/// 由有效费用样本计算日均（纯函数，供测试）。
pub fn daily_avg_from(history: &[DailyUsage]) -> f64 {
    let valid: Vec<&DailyUsage> = history.iter().filter(|d| d.cost.is_some()).collect();
    let sum: f64 = valid.iter().filter_map(|d| d.cost).sum();
    if valid.is_empty() {
        0.0
    } else {
        sum / valid.len() as f64 // 有数据日平均
    }
}

/// 计算指定 Provider 的消耗预测（核心逻辑，供命令与刷新提醒复用）。
pub fn predict_for(db: &Db, provider_id: i64, days: u64) -> Result<Option<Prediction>, AppError> {
    let days = days.clamp(1, 365);
    // P2：Provider 不存在时返回 None（而非空预测/报错）
    let exists: bool = db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT 1 FROM providers WHERE id = ?1",
                [provider_id],
                |_| Ok(true),
            )
            .optional()
        })?
        .unwrap_or(false);
    if !exists {
        return Ok(None);
    }
    // 历史行查询（含今天的 N 天窗口）
    let history: Vec<DailyUsage> = db
        .with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT date, tokens, today_tokens, cost, balance FROM usage_history
                 WHERE provider_id = ?1 AND date >= date('now', ?2)
                 ORDER BY date",
            )?;
            let rows = stmt.query_map(
                rusqlite::params![provider_id, history_start_offset(days)],
                |row| {
                    Ok(DailyUsage {
                        date: row.get(0)?,
                        tokens: row.get::<_, i64>(1)? as u64,
                        today_tokens: row.get(2)?,
                        cost: row.get(3)?,
                        balance: row.get(4)?,
                    })
                },
            )?;
            rows.collect()
        })
        .map_err(AppError::from)?;
    let daily_avg = daily_avg_from(&history);
    // 当前余额 = 最新一天记录的 balance
    let balance = history.last().and_then(|d| d.balance);
    let (days_left, exhausted_date) = match (balance, daily_avg) {
        (Some(b), avg) if avg > 0.0 && b > 0.0 => {
            let d = (b / avg).max(0.0);
            // P2：耗尽日期按 ceil 取整（1.9 天 → 2 天后），避免向下截断导致日期偏早
            let whole_days = d.ceil() as i64;
            let date = chrono::Utc::now()
                .date_naive()
                .checked_add_signed(chrono::Duration::days(whole_days));
            (Some(d), date.map(|x| x.to_string()))
        }
        _ => (None, None),
    };
    Ok(Some(Prediction {
        daily_cost_avg: daily_avg,
        samples: history.iter().filter(|d| d.cost.is_some()).count(),
        days_span: days,
        balance,
        days_left,
        exhausted_date,
    }))
}

/// 计算指定 Provider 的消耗预测（Tauri command 包装）。
#[tauri::command]
pub fn get_prediction(
    db: State<'_, Db>,
    provider_id: i64,
    days: Option<u64>,
) -> Result<Option<Prediction>, AppError> {
    predict_for(&db, provider_id, days.unwrap_or(7))
}

/// 额度提醒级别（mission.md §11：>50% 正常 / <30% 黄 / <10% 红）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AlertLevel {
    None,
    Warning,  // <30%
    Critical, // <10%
}

/// 根据剩余百分比与预测剩余天数计算提醒级别。
/// V0.5 复审 P1：remaining 存在时以百分比为准（额度充足则不提醒，不回落天数）；
/// 无 remaining 时用预测剩余天数兜底（<3 天红 / <7 天黄）。
pub fn alert_level_for(remaining_percent: Option<f64>, days_left: Option<f64>) -> AlertLevel {
    if let Some(p) = remaining_percent {
        if p < 10.0 {
            return AlertLevel::Critical;
        }
        if p < 30.0 {
            return AlertLevel::Warning;
        }
        return AlertLevel::None; // 百分比正常即不再看天数（百分比优先）
    }
    if let Some(d) = days_left {
        if d < 3.0 {
            return AlertLevel::Critical;
        }
        if d < 7.0 {
            return AlertLevel::Warning;
        }
    }
    AlertLevel::None
}

/// 提醒去重状态（provider_id → 上次级别），级别提升才通知。
#[derive(Default)]
pub struct AlertState(pub std::sync::Mutex<std::collections::HashMap<i64, AlertLevel>>);

/// 刷新成功后检查额度并发送系统通知（mission.md §11）。
/// 判定：优先 remaining 百分比；无百分比时用预测剩余天数（days_left）。
/// 仅在级别提升（None→Warning/Critical、Warning→Critical）时通知，恢复后重置。
pub fn check_alerts(
    app: &tauri::AppHandle,
    state: &AlertState,
    provider: &crate::providers::ProviderConfig,
    usage: &ProviderUsage,
    days_left: Option<f64>,
) {
    let level = alert_level_for(usage.remaining, days_left);
    let mut map = state.0.lock().unwrap();
    let prev = map.get(&provider.id).copied().unwrap_or(AlertLevel::None);
    if level > prev {
        if level != AlertLevel::None {
            use tauri_plugin_notification::NotificationExt;
            let (title, body) = match level {
                AlertLevel::Critical => match usage.remaining {
                    Some(p) => (
                        "AI API Monitor：额度严重不足",
                        format!("{} 剩余额度不足 10%（{:.1}%）", provider.name, p),
                    ),
                    None => (
                        "AI API Monitor：额度严重不足",
                        format!(
                            "{} 预计剩余不足 3 天（{:.1} 天）",
                            provider.name,
                            days_left.unwrap_or(0.0)
                        ),
                    ),
                },
                AlertLevel::Warning => match usage.remaining {
                    Some(p) => (
                        "AI API Monitor：额度偏低",
                        format!("{} 剩余额度低于 30%（{:.1}%）", provider.name, p),
                    ),
                    None => (
                        "AI API Monitor：额度偏低",
                        format!(
                            "{} 预计剩余不足 7 天（{:.1} 天）",
                            provider.name,
                            days_left.unwrap_or(0.0)
                        ),
                    ),
                },
                AlertLevel::None => return,
            };
            // P2：仅通知发送成功才记录已通知级别；失败写日志（保留旧级别以便下次重试）
            match app.notification().builder().title(title).body(body).show() {
                Ok(()) => {
                    map.insert(provider.id, level);
                }
                Err(e) => {
                    crate::security::safe_log(
                        "check_alerts",
                        format!(
                            "通知发送失败（provider={}，level={level:?}）: {e}",
                            provider.name
                        ),
                    );
                }
            }
        }
    } else if level < prev {
        map.insert(provider.id, level); // 已充值/恢复：重置，下次跌破再提醒
    }
}

/// 读取 DIY 布局 JSON（V0.3；未设置时返回 None）。
#[tauri::command]
pub fn get_layout(db: State<'_, Db>) -> Result<Option<String>, AppError> {
    settings::get_setting(&db, settings::SETTING_LAYOUT)
}

// ---- V1.0 自动更新 ----

/// 更新检查结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
}

fn validate_updater_security_config() -> Result<(), AppError> {
    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
        .map_err(|_| AppError::Invalid("更新器安全配置无效".into()))?;
    let updater = &config["plugins"]["updater"];
    let public_key = updater["pubkey"].as_str().unwrap_or("").trim();
    let endpoints = updater["endpoints"].as_array().cloned().unwrap_or_default();
    if public_key.is_empty() || endpoints.is_empty() {
        return Err(AppError::Invalid(
            "自动更新已安全禁用：尚未配置签名公钥与 HTTPS 更新源".into(),
        ));
    }
    for endpoint in endpoints {
        let value = endpoint
            .as_str()
            .ok_or_else(|| AppError::Invalid("更新源格式无效".into()))?;
        let url =
            url::Url::parse(value).map_err(|_| AppError::Invalid("更新源 URL 无效".into()))?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err(AppError::Invalid(
                "自动更新仅允许无内嵌凭据的 HTTPS 更新源".into(),
            ));
        }
    }
    Ok(())
}

/// 检查是否有可用更新（需要发布前配置 updater 签名与更新源）。
fn validate_update_transition(
    current_version: &str,
    expected_version: &str,
    available_version: &str,
) -> Result<(), AppError> {
    if expected_version != available_version {
        return Err(AppError::Invalid(
            "The available update changed. Check again before installing.".into(),
        ));
    }
    let parse = |value: &str| {
        semver::Version::parse(value.trim_start_matches(['v', 'V']))
            .map_err(|_| AppError::Invalid("Invalid update version.".into()))
    };
    let current = parse(current_version)?;
    let available = parse(available_version)?;
    if available <= current {
        return Err(AppError::Invalid(
            "Update downgrade or reinstall was blocked.".into(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateInfo, AppError> {
    use tauri_plugin_updater::UpdaterExt;
    validate_updater_security_config()?;
    let updater = app
        .updater()
        .map_err(|e| AppError::Invalid(format!("更新器未配置（发布前需配置签名与更新源）: {e}")))?;
    match updater.check().await {
        Ok(Some(update)) => {
            validate_update_transition(
                env!("CARGO_PKG_VERSION"),
                &update.version,
                &update.version,
            )?;
            Ok(UpdateInfo {
                available: true,
                version: Some(update.version.clone()),
                notes: update.body.clone(),
            })
        }
        Ok(None) => Ok(UpdateInfo {
            available: false,
            version: None,
            notes: None,
        }),
        Err(e) => Err(AppError::Invalid(format!("更新检查失败: {e}"))),
    }
}

/// 下载并安装可用更新（安装完成后通常需要重启应用）。
#[tauri::command]
pub async fn install_update(app: AppHandle, expected_version: String) -> Result<String, AppError> {
    use tauri_plugin_updater::UpdaterExt;
    validate_updater_security_config()?;
    if expected_version.len() > 64
        || !expected_version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        return Err(AppError::Invalid("待安装版本号格式无效".into()));
    }
    let updater = app
        .updater()
        .map_err(|e| AppError::Invalid(format!("更新器未配置: {e}")))?;
    let update = updater
        .check()
        .await
        .map_err(|e| AppError::Invalid(format!("更新检查失败: {e}")))?
        .ok_or_else(|| AppError::Invalid("当前没有可用更新".into()))?;
    validate_update_transition(
        env!("CARGO_PKG_VERSION"),
        &expected_version,
        &update.version,
    )?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| AppError::Invalid(format!("更新下载/安装失败: {e}")))?;
    Ok(format!("已更新到 v{}，请重启应用生效", update.version))
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
    let theme = value
        .get("theme")
        .and_then(|t| t.as_str())
        .unwrap_or("dark");
    if theme != "dark" && theme != "light" {
        return Err("theme 仅支持 dark / light".into());
    }
    if let Some(overrides) = value.get("themeOverrides") {
        let Some(map) = overrides.as_object() else {
            return Err("themeOverrides 必须为对象".into());
        };
        const ALLOWED: [&str; 15] = [
            "accent",
            "accent-dim",
            "accent-contrast",
            "surface",
            "card",
            "card-hover",
            "control",
            "control-hover",
            "border",
            "text-primary",
            "text-secondary",
            "text-muted",
            "success",
            "warning",
            "danger",
        ];
        if map.len() > ALLOWED.len() {
            return Err("themeOverrides 颜色数量超限".into());
        }
        for (key, value) in map {
            if !ALLOWED.contains(&key.as_str()) {
                return Err(format!("未知主题颜色: {key}"));
            }
            let Some(color) = value.as_str() else {
                return Err(format!("主题颜色 {key} 必须为字符串"));
            };
            let valid = color.len() == 7
                && color.starts_with('#')
                && color[1..].bytes().all(|b| b.is_ascii_hexdigit());
            if !valid {
                return Err(format!("主题颜色 {key} 必须为 #RRGGBB"));
            }
        }
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
        if !["providers", "summary", "cost", "trend"].contains(&ty) {
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
pub fn validate_refresh_intervals(
    foreground_secs: u64,
    background_secs: u64,
) -> Result<(), String> {
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
    validate_refresh_intervals(foreground_secs, background_secs).map_err(AppError::Invalid)?;
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

#[tauri::command]
pub fn snap_window_to_work_area(app: AppHandle) -> Result<(), AppError> {
    window_mode::snap_window_to_work_area(&app)
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
    let adapter = manager.get(&provider.provider_type).ok_or_else(|| {
        AppError::Invalid(format!(
            "不支持的 Provider 类型: {}",
            provider.provider_type
        ))
    })?;
    // Codex 只查询 CLI 的公开登录状态，不读取凭据或 auth 文件。
    let api_key =
        if provider.credential_source() == crate::providers::CredentialSource::PublicCliStatus {
            zeroize::Zeroizing::new(String::new())
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
    fn alert_level_thresholds() {
        use super::AlertLevel;
        assert_eq!(super::alert_level_for(None, None), AlertLevel::None);
        assert_eq!(super::alert_level_for(Some(50.0), None), AlertLevel::None);
        assert_eq!(super::alert_level_for(Some(30.0), None), AlertLevel::None); // 边界 30% 属正常
        assert_eq!(
            super::alert_level_for(Some(29.9), None),
            AlertLevel::Warning
        );
        assert_eq!(
            super::alert_level_for(Some(10.0), None),
            AlertLevel::Warning
        ); // 边界 10% 属警告
        assert_eq!(
            super::alert_level_for(Some(9.9), None),
            AlertLevel::Critical
        );
        // 无百分比时按预测剩余天数兜底（V0.5 复审 P1）
        assert_eq!(
            super::alert_level_for(None, Some(2.9)),
            AlertLevel::Critical
        );
        assert_eq!(super::alert_level_for(None, Some(5.0)), AlertLevel::Warning);
        assert_eq!(super::alert_level_for(None, Some(7.0)), AlertLevel::None); // 边界 7 天属正常
        assert_eq!(super::alert_level_for(None, None), AlertLevel::None);
        // 百分比优先于天数
        assert_eq!(
            super::alert_level_for(Some(50.0), Some(2.0)),
            AlertLevel::None
        );
    }

    #[test]
    fn history_window_is_inclusive_of_today() {
        // V0.5 复审 P1：N 天窗口 = -(N-1) 天，避免多取一天
        assert_eq!(super::history_start_offset(7), "-6 days");
        assert_eq!(super::history_start_offset(30), "-29 days");
        assert_eq!(super::history_start_offset(1), "-0 days");
        assert_eq!(super::history_start_offset(0), "-0 days"); // saturating
    }

    #[test]
    fn daily_avg_uses_only_valid_cost_samples() {
        use super::DailyUsage;
        let mk = |cost: Option<f64>| DailyUsage {
            date: "2025-08-01".into(),
            tokens: 0,
            today_tokens: None,
            cost,
            balance: None,
        };
        // 有效样本 2 个（1.0 + 3.0）/2 = 2.0；NULL 视为未知不参与
        let h = vec![mk(Some(1.0)), mk(None), mk(Some(3.0))];
        assert!((super::daily_avg_from(&h) - 2.0).abs() < 1e-9);
        // 全 NULL → 0
        assert_eq!(super::daily_avg_from(&[mk(None)]), 0.0);
    }

    #[test]
    fn prediction_exhaustion_date_ceils_days() {
        // P2：1.9 天应取整为 2 天后，避免向下截断偏早
        let today = chrono::Utc::now().date_naive();
        let whole = (1.9_f64).ceil() as i64;
        let date = today
            .checked_add_signed(chrono::Duration::days(whole))
            .unwrap();
        assert_eq!(date, today + chrono::Duration::days(2));
    }

    #[test]
    fn layout_validation_accepts_valid_json() {
        let ok = r#"{"theme":"dark","widgets":[{"id":"w1","type":"providers","visible":true}]}"#;
        assert!(super::validate_layout_json(ok).is_ok());
        let ok_light = r#"{"theme":"light","widgets":[]}"#;
        assert!(super::validate_layout_json(ok_light).is_ok());
        let full_palette = r##"{"theme":"light","widgets":[],"themeOverrides":{"accent":"#336699","accent-dim":"#224466","accent-contrast":"#ffffff","surface":"#eef3f7","card":"#ffffff","card-hover":"#e5edf3","control":"#f4f7fa","control-hover":"#e8eef3","border":"#7890a0","text-primary":"#101820","text-secondary":"#405060","text-muted":"#607080","success":"#087f5b","warning":"#a65f00","danger":"#c92a45"}}"##;
        assert!(super::validate_layout_json(full_palette).is_ok());
    }

    #[test]
    fn layout_validation_rejects_bad_input() {
        assert!(super::validate_layout_json("not json").is_err());
        assert!(super::validate_layout_json(r#"{"theme":"blue","widgets":[]}"#).is_err());
        assert!(super::validate_layout_json(r#"{"theme":"dark","widgets":{}}"#).is_err());
        assert!(super::validate_layout_json(
            r##"{"theme":"dark","widgets":[],"themeOverrides":{"unknown":"#ffffff"}}"##
        )
        .is_err());
        assert!(super::validate_layout_json(
            r#"{"theme":"dark","widgets":[],"themeOverrides":{"accent":"red"}}"#
        )
        .is_err());
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
    }
}

#[cfg(test)]
mod codex_url_tests {
    use crate::providers::ProviderManager;
    use crate::settings::validate_provider_input;

    #[test]
    fn codex_accepts_exact_official_url() {
        let m = ProviderManager::new();
        assert!(
            validate_provider_input(&m, "codex", "https://chatgpt.com/backend-api/codex").is_ok()
        );
        assert!(
            validate_provider_input(&m, "codex", "https://chatgpt.com/backend-api/codex/").is_ok()
        );
    }

    #[test]
    fn codex_rejects_any_other_url() {
        let m = ProviderManager::new();
        // 恶意 host
        assert!(validate_provider_input(&m, "codex", "https://evil.example.com").is_err());
        // 子域伪装（chatgpt.com.evil.test）
        assert!(validate_provider_input(
            &m,
            "codex",
            "https://chatgpt.com.evil.test/backend-api/codex"
        )
        .is_err());
        // 非默认端口
        assert!(
            validate_provider_input(&m, "codex", "https://chatgpt.com:8443/backend-api/codex")
                .is_err()
        );
        // userinfo
        assert!(validate_provider_input(
            &m,
            "codex",
            "https://user:pass@chatgpt.com/backend-api/codex"
        )
        .is_err());
        // 路径混淆
        assert!(validate_provider_input(
            &m,
            "codex",
            "https://chatgpt.com/backend-api/codex/../codex"
        )
        .is_err());
        // 非 https
        assert!(
            validate_provider_input(&m, "codex", "http://chatgpt.com/backend-api/codex").is_err()
        );
    }
}

#[cfg(test)]
mod updater_security_tests {
    #[test]
    fn updater_is_disabled_until_signed_https_config_exists() {
        let result = super::validate_updater_security_config();
        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("安全禁用"));
    }

    #[test]
    fn update_transition_allows_only_a_confirmed_upgrade() {
        assert!(super::validate_update_transition("1.2.3", "1.2.4", "1.2.4").is_ok());
        assert!(super::validate_update_transition("1.2.3", "1.2.3", "1.2.3").is_err());
        assert!(super::validate_update_transition("1.2.3", "1.2.2", "1.2.2").is_err());
        assert!(super::validate_update_transition("1.2.3", "1.2.4", "1.2.5").is_err());
        assert!(super::validate_update_transition("1.2.3", "latest", "latest").is_err());
    }
}

/// 将一次用量写入 usage_history（按 provider+date UPSERT，供日报/周报/月报）。
fn record_usage(db: &Db, provider: &ProviderConfig, usage: &ProviderUsage) -> Result<(), AppError> {
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let raw = serde_json::to_string(usage).unwrap_or_default();
    // V0.5 口径：tokens=累计快照（兼容历史）；today_tokens 直接落 usage.today_tokens
    // （Some 含真实 0；None=平台不提供）；cost 为 None 时落 NULL，不再伪装成 0。
    let today_tokens = usage.today_tokens.map(|t| t as i64);
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO usage_history (provider_id, date, tokens, today_tokens, cost, balance, raw_json, created_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(provider_id, date) DO UPDATE SET
               tokens = excluded.tokens,
               today_tokens = excluded.today_tokens,
               cost = excluded.cost,
               balance = excluded.balance,
               raw_json = excluded.raw_json",
            rusqlite::params![
                provider.id,
                date,
                usage.total_tokens as i64,
                today_tokens,
                usage.today_cost,
                usage.balance,
                raw,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    })
    .map_err(AppError::from)
}
