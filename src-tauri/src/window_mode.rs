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
use tauri::{Emitter, LogicalSize, Manager};

/// 主窗口标签名（与 lib.rs 保持一致）。
const MAIN_WINDOW: &str = "main";

/// 窗口模式/置顶变更事件名（Rust → 前端状态同步）。
pub const EVENT_WINDOW_STATE_CHANGED: &str = "window-mode-changed";

/// settings 键名。
pub const SETTING_WINDOW_MODE: &str = "window.mode";
pub const SETTING_ALWAYS_ON_TOP: &str = "window.alwaysOnTop";
/// 各模式窗口几何（逻辑坐标 JSON：{width,height,x,y}），P2 持久化。
const SETTING_GEOMETRY_FULL: &str = "window.geometryFull";
const SETTING_POS_MINI: &str = "window.posMini";
const SETTING_POS_BALL: &str = "window.posBall";

/// 窗口几何（逻辑像素）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Geometry {
    width: f64,
    height: f64,
    x: f64,
    y: f64,
}

/// 保存指定模式的窗口几何：Full 存尺寸+位置，Mini/Ball 存位置（尺寸固定）。
/// 显式传入模式，避免切换瞬间 current_state 读到旧模式导致写错 key（review should-fix）。
pub fn save_geometry_for(app: &tauri::AppHandle, db: &Db, mode: WindowMode) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let (Ok(outer), Ok(size), Ok(sf)) = (
        window.outer_position(),
        window.outer_size(),
        window.scale_factor(),
    ) else {
        return;
    };
    let geo = Geometry {
        width: size.width as f64 / sf,
        height: size.height as f64 / sf,
        x: outer.x as f64 / sf,
        y: outer.y as f64 / sf,
    };
    let key = match mode {
        WindowMode::Full => SETTING_GEOMETRY_FULL,
        WindowMode::Mini => SETTING_POS_MINI,
        WindowMode::Ball => SETTING_POS_BALL,
    };
    if let Ok(json) = serde_json::to_string(&geo) {
        let _ = set_setting(db, key, &json);
    }
}

/// 保存当前模式的窗口几何（供 moved/resized 监听使用）。
pub fn save_current_geometry(app: &tauri::AppHandle, db: &Db) {
    let mode = current_state(db).mode;
    save_geometry_for(app, db, mode);
}

/// 读取某模式的已保存几何。
fn load_geometry(db: &Db, key: &str) -> Option<Geometry> {
    get_setting(db, key)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
}

/// 各模式的目标逻辑尺寸。
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
/// 持久化失败时补偿恢复原生窗口状态，保证与数据库一致（P1 修复）。
pub fn apply_mode(
    app: &tauri::AppHandle,
    db: &Db,
    mode: WindowMode,
) -> Result<(), AppError> {
    let old_mode = current_state(db).mode; // 写入前的持久化模式（用于补偿）
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
    if let Err(e) = set_setting(db, SETTING_WINDOW_MODE, mode.label()) {
        // 补偿：恢复旧模式的尺寸与可缩放性
        let (ow, oh) = old_mode.size();
        let _ = window.set_size(LogicalSize::new(ow, oh));
        let _ = window.set_resizable(old_mode == WindowMode::Full);
        return Err(e.into());
    }
    // 通知前端同步视图（托盘路径与命令路径统一状态源）
    let _ = app.emit(EVENT_WINDOW_STATE_CHANGED, current_state(db));
    // 以明确的目标模式保存几何（避免切换瞬间按旧模式写错 key）
    save_geometry_for(app, db, mode);
    Ok(())
}

/// 设置 Always On Top 并持久化。
/// 持久化失败时补偿恢复原置顶状态（P1 修复）。
pub fn set_always_on_top(app: &tauri::AppHandle, db: &Db, enabled: bool) -> Result<(), AppError> {
    let old_enabled = current_state(db).always_on_top;
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| AppError::Invalid("主窗口不存在".into()))?;
    window
        .set_always_on_top(enabled)
        .map_err(|e| AppError::Invalid(format!("设置置顶失败: {e}")))?;
    if let Err(e) = set_setting(db, SETTING_ALWAYS_ON_TOP, if enabled { "1" } else { "0" }) {
        // 补偿：恢复原置顶状态
        let _ = window.set_always_on_top(old_enabled);
        return Err(e.into());
    }
    // 通知前端同步（Settings 开关与事件保持一致）
    let _ = app.emit(EVENT_WINDOW_STATE_CHANGED, current_state(db));
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

/// 启动时恢复窗口模式、几何（尺寸/位置）与置顶设置（P2 增强）。
pub fn restore_window_state(app: &tauri::App) -> tauri::Result<()> {
    let db = app.state::<Db>();
    let state = current_state(&db);
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| tauri::Error::WindowNotFound)?;

    // 尺寸：Full 用用户上次保存的尺寸，Mini/Ball 固定
    let (w, h) = match state.mode {
        WindowMode::Full => load_geometry(&db, SETTING_GEOMETRY_FULL)
            .map(|g| (g.width, g.height))
            .unwrap_or((460.0, 720.0)),
        WindowMode::Mini => WindowMode::Mini.size(),
        WindowMode::Ball => WindowMode::Ball.size(),
    };
    window.set_size(LogicalSize::new(w, h))?;
    window.set_resizable(state.mode == WindowMode::Full)?;

    // 位置：各模式独立恢复（坐标非负才应用，越界交由系统校正）
    let pos_key = match state.mode {
        WindowMode::Full => SETTING_GEOMETRY_FULL,
        WindowMode::Mini => SETTING_POS_MINI,
        WindowMode::Ball => SETTING_POS_BALL,
    };
    if let Some(g) = load_geometry(&db, pos_key) {
        if g.x >= 0.0 && g.y >= 0.0 {
            let sf = window.scale_factor().unwrap_or(1.0);
            let _ = window.set_position(tauri::PhysicalPosition::new(
                (g.x * sf) as i32,
                (g.y * sf) as i32,
            ));
        }
    }

    // 置顶
    window.set_always_on_top(state.always_on_top)?;
    Ok(())
}
