//! 窗口状态机：Full / Mini / Ball 三模式（V0.2，对应 mission.md §10）
//!
//! - Full：完整 Dashboard（460×720）
//! - Mini：紧凑状态条（280×96）
//! - Ball：悬浮小球（72×72）
//!
//! 模式与 Always On Top 均持久化到 settings 表，启动时恢复。

use crate::db::Db;
use crate::settings::{get_setting, set_setting, AppError};
use serde::{Deserialize, Serialize};
use tauri::{LogicalSize, Manager};

/// 主窗口标签名（与 lib.rs 保持一致）。
const MAIN_WINDOW: &str = "main";

/// settings 键名。
pub const SETTING_WINDOW_MODE: &str = "window.mode";
pub const SETTING_ALWAYS_ON_TOP: &str = "window.alwaysOnTop";

/// 窗口模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowMode {
    Full,
    Mini,
    Ball,
}

/// 窗口状态（返回给前端）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub mode: WindowMode,
    pub always_on_top: bool,
}

impl WindowMode {
    fn label(self) -> &'static str {
        match self {
            WindowMode::Full => "full",
            WindowMode::Mini => "mini",
            WindowMode::Ball => "ball",
        }
    }

    fn from_label(s: &str) -> Option<Self> {
        match s {
            "full" => Some(WindowMode::Full),
            "mini" => Some(WindowMode::Mini),
            "ball" => Some(WindowMode::Ball),
            _ => None,
        }
    }

    /// 各模式的目标逻辑尺寸。
    fn size(self) -> (f64, f64) {
        match self {
            WindowMode::Full => (460.0, 720.0),
            WindowMode::Mini => (280.0, 96.0),
            WindowMode::Ball => (72.0, 72.0),
        }
    }
}

/// 应用窗口模式（尺寸 + 可缩放性 + 持久化）。
pub fn apply_mode(
    app: &tauri::AppHandle,
    db: &Db,
    mode: WindowMode,
) -> Result<(), AppError> {
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| AppError::Invalid("主窗口不存在".into()))?;
    let (w, h) = mode.size();
    window
        .set_size(LogicalSize::new(w, h))
        .map_err(|e| AppError::Invalid(format!("调整窗口尺寸失败: {e}")))?;
    // Mini/Ball 固定尺寸，Full 可缩放
    window
        .set_resizable(mode == WindowMode::Full)
        .map_err(|e| AppError::Invalid(format!("设置窗口可缩放性失败: {e}")))?;
    set_setting(db, SETTING_WINDOW_MODE, mode.label())?;
    Ok(())
}

/// 设置 Always On Top 并持久化。
pub fn set_always_on_top(app: &tauri::AppHandle, db: &Db, enabled: bool) -> Result<(), AppError> {
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| AppError::Invalid("主窗口不存在".into()))?;
    window
        .set_always_on_top(enabled)
        .map_err(|e| AppError::Invalid(format!("设置置顶失败: {e}")))?;
    set_setting(db, SETTING_ALWAYS_ON_TOP, if enabled { "1" } else { "0" })?;
    Ok(())
}

/// 读取当前窗口状态（来自持久化设置）。
pub fn current_state(db: &Db) -> WindowState {
    let mode = get_setting(db, SETTING_WINDOW_MODE)
        .ok()
        .flatten()
        .and_then(|s| WindowMode::from_label(&s))
        .unwrap_or(WindowMode::Full);
    let always_on_top = get_setting(db, SETTING_ALWAYS_ON_TOP)
        .ok()
        .flatten()
        .map(|s| s == "1")
        .unwrap_or(false);
    WindowState {
        mode,
        always_on_top,
    }
}

/// 启动时恢复窗口模式与置顶设置。
pub fn restore_window_state(app: &tauri::App) -> tauri::Result<()> {
    let db = app.state::<Db>();
    let state = current_state(&db);
    // 应用模式（尺寸）
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| tauri::Error::WindowNotFound)?;
    let (w, h) = state.mode.size();
    window.set_size(LogicalSize::new(w, h))?;
    window.set_resizable(state.mode == WindowMode::Full)?;
    // 置顶
    window.set_always_on_top(state.always_on_top)?;
    Ok(())
}
