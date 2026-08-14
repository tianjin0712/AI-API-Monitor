//! 窗口状态机：Full / Mini / Ball 三模式（V0.2，对应 mission.md §10）
//!
//! - Full：完整 Dashboard（460×720）
//! - Mini：紧凑状态条（280×96）
//! - Ball：悬浮方块（96×96，与 Mini 高度一致）
//!
//! 模式与 Always On Top 均持久化到 settings 表，启动时恢复。

use crate::db::Db;
use crate::settings::{get_setting, set_setting, AppError};
use serde::{Deserialize, Serialize};
use std::{thread, time::Duration};
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition};

/// 主窗口标签名（与 lib.rs 保持一致）。
const MAIN_WINDOW: &str = "main";
/*
pub fn show_floating_tooltip(app: &tauri::AppHandle, kind: &str) -> Result<(), AppError> {
    let (label, width) = match kind {
        "expand" => (TOOLTIP_EXPAND, 78.0),
        "collapse" => (TOOLTIP_COLLAPSE, 118.0),
        _ => return Err(AppError::Invalid("无效的悬浮提示类型".into())),
    };
    let main = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| AppError::Invalid("主窗口不存在".into()))?;
    let tooltip = if let Some(window) = app.get_webview_window(label) {
        window
    } else {
        WebviewWindowBuilder::new(
            app,
            label,
            WebviewUrl::App(format!("index.html?floating-tooltip={kind}").into()),
        )
        .title("")
        .inner_size(width, 34.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .visible(false)
        .build()
        .map_err(|error| AppError::Invalid(format!("创建悬浮提示失败: {error}")))?
    };
    let position = main
        .outer_position()
        .map_err(|error| AppError::Invalid(format!("读取窗口位置失败: {error}")))?;
    let size = main
        .outer_size()
        .map_err(|error| AppError::Invalid(format!("读取窗口尺寸失败: {error}")))?;
    let monitor = main
        .current_monitor()
        .map_err(|error| AppError::Invalid(format!("读取显示器失败: {error}")))?
        .ok_or_else(|| AppError::Invalid("未找到当前显示器".into()))?;
    let scale = monitor.scale_factor();
    let tooltip_width = (width * scale).round() as i32;
    let tooltip_height = (34.0 * scale).round() as i32;
    let gap = (8.0 * scale).round() as i32;
    let work = monitor.work_area();
    let work_right = work.position.x + work.size.width as i32;
    let work_bottom = work.position.y + work.size.height as i32;
    let x = (position.x + (size.width as i32 - tooltip_width) / 2)
        .clamp(work.position.x, work_right - tooltip_width);
    let below = position.y + size.height as i32 + gap;
    let y = if below + tooltip_height <= work_bottom {
        below
    } else {
        (position.y - tooltip_height - gap).max(work.position.y)
    };
    tooltip
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| AppError::Invalid(format!("定位悬浮提示失败: {error}")))?;
    tooltip
        .show()
        .map_err(|error| AppError::Invalid(format!("显示悬浮提示失败: {error}")))?;
    let _ = tooltip.emit("floating-tooltip-show", ());
    Ok(())
}

pub fn hide_floating_tooltip(app: &tauri::AppHandle) {
    let mut windows = Vec::new();
    for label in [TOOLTIP_EXPAND, TOOLTIP_COLLAPSE] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.emit("floating-tooltip-hide", ());
            windows.push(window);
        }
    }
    if !windows.is_empty() {
        thread::sleep(Duration::from_millis(130));
        for window in windows {
            let _ = window.hide();
        }
    }
}
*/

/// 窗口模式/置顶变更事件名（Rust → 前端状态同步）。
pub const EVENT_WINDOW_STATE_CHANGED: &str = "window-mode-changed";

/// settings 键名。
pub const SETTING_WINDOW_MODE: &str = "window.mode";
pub const SETTING_ALWAYS_ON_TOP: &str = "window.alwaysOnTop";
/// 各模式窗口几何（逻辑坐标 JSON：{width,height,x,y}）。
/// 宽高字段仅为旧数据兼容，恢复时只使用位置，尺寸始终采用模式预设值。
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

fn rects_intersect(rect: Geometry, monitor: Geometry) -> bool {
    rect.width > 0.0
        && rect.height > 0.0
        && rect.x < monitor.x + monitor.width
        && rect.x + rect.width > monitor.x
        && rect.y < monitor.y + monitor.height
        && rect.y + rect.height > monitor.y
}

fn aligned_axis(
    source_start: i32,
    source_size: i32,
    target_size: i32,
    monitor_start: i32,
    monitor_size: i32,
    edge_threshold: i32,
) -> i32 {
    let monitor_end = monitor_start + monitor_size;
    let left_gap = source_start - monitor_start;
    let right_gap = monitor_end - (source_start + source_size);
    if left_gap <= edge_threshold {
        monitor_start
    } else if right_gap <= edge_threshold {
        monitor_end - target_size
    } else {
        source_start
            .min(monitor_end - target_size)
            .max(monitor_start)
    }
}

/// Clamp a window axis to the current monitor work area.  Coordinates are
/// physical pixels so this remains correct on mixed-DPI and negative-origin
/// monitors.
fn clamp_axis(source_start: i32, target_size: i32, work_start: i32, work_size: i32) -> i32 {
    let max_start = work_start + work_size - target_size;
    if max_start < work_start {
        work_start
    } else {
        source_start.clamp(work_start, max_start)
    }
}

fn snapped_axis(
    source_start: i32,
    target_size: i32,
    work_start: i32,
    work_size: i32,
    threshold: i32,
) -> i32 {
    let work_end = work_start + work_size;
    let max_start = work_end - target_size;
    if max_start < work_start {
        return work_start;
    }
    // Measure the actual window edges, not just the top-left coordinate.
    // This is important for right/bottom edges and mixed-DPI monitors.
    let leading_gap = source_start - work_start;
    let trailing_gap = work_end - (source_start + target_size);
    if leading_gap >= 0 && leading_gap <= threshold {
        work_start
    } else if trailing_gap >= 0 && trailing_gap <= threshold {
        max_start
    } else {
        source_start.clamp(work_start, max_start)
    }
}

fn is_geometry_visible(window: &tauri::WebviewWindow, geometry: Geometry, scale: f64) -> bool {
    let Ok(monitors) = window.available_monitors() else {
        return false;
    };
    monitors.iter().any(|monitor| {
        let work = monitor.work_area();
        rects_intersect(
            Geometry {
                x: geometry.x * scale,
                y: geometry.y * scale,
                width: geometry.width * scale,
                height: geometry.height * scale,
            },
            Geometry {
                x: work.position.x as f64,
                y: work.position.y as f64,
                width: work.size.width as f64,
                height: work.size.height as f64,
            },
        )
    })
}

/// 保存指定模式的窗口几何；所有模式只恢复位置，尺寸均固定。
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
            WindowMode::Ball => (96.0, 96.0),
        }
    }
}

/// 应用窗口模式（固定尺寸 + 持久化）。
/// 持久化失败时补偿恢复原生窗口状态，保证与数据库一致（P1 修复）。
pub fn apply_mode(app: &tauri::AppHandle, db: &Db, mode: WindowMode) -> Result<(), AppError> {
    let old_mode = current_state(db).mode; // 写入前的持久化模式（用于补偿）
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| AppError::Invalid("主窗口不存在".into()))?;
    save_geometry_for(app, db, old_mode);
    let source_geometry = match old_mode {
        WindowMode::Full => load_geometry(db, SETTING_GEOMETRY_FULL),
        WindowMode::Mini => load_geometry(db, SETTING_POS_MINI),
        WindowMode::Ball => load_geometry(db, SETTING_POS_BALL),
    };
    let old_position = window.outer_position().ok();
    let old_size = window.outer_size().ok();
    let scale = window.scale_factor().unwrap_or(1.0);
    let monitor = window.current_monitor().ok().flatten();
    let (w, h) = mode.size();
    // 所有模式都禁止最大化、全屏与手动拉伸；切换时先退出遗留状态，
    // 再恢复各模式的唯一预设尺寸，避免页面被系统铺满或扯开。
    window
        .set_fullscreen(false)
        .map_err(|e| AppError::Invalid(format!("退出全屏失败: {e}")))?;
    window
        .unmaximize()
        .map_err(|e| AppError::Invalid(format!("退出最大化失败: {e}")))?;
    window
        .set_maximizable(false)
        .map_err(|e| AppError::Invalid(format!("禁用最大化失败: {e}")))?;
    window
        .set_resizable(false)
        .map_err(|e| AppError::Invalid(format!("锁定窗口尺寸失败: {e}")))?;
    let animate_compact_resize = matches!(
        (old_mode, mode),
        (WindowMode::Ball, WindowMode::Mini) | (WindowMode::Mini, WindowMode::Ball)
    );
    if animate_compact_resize {
        // Ball 与 Mini 共用左侧 GIF 槽位：只改变右边界，绝不让系统根据
        // 新尺寸重算窗口中心，避免角色在抽屉动画里抖动或位移。
        if let Some(position) = old_position {
            let _ = window.set_position(position);
        }
        let (start_w, start_h) = old_mode.size();
        let target_position = old_position.and_then(|position| {
            monitor.as_ref().map(|monitor| {
                let work = monitor.work_area();
                PhysicalPosition::new(
                    clamp_axis(
                        position.x,
                        (w * scale).round() as i32,
                        work.position.x,
                        work.size.width as i32,
                    ),
                    clamp_axis(
                        position.y,
                        (h * scale).round() as i32,
                        work.position.y,
                        work.size.height as i32,
                    ),
                )
            })
        });
        for frame in 1..=12 {
            let progress = frame as f64 / 12.0;
            let eased = progress * progress * (3.0 - 2.0 * progress);
            let frame_w = start_w + (w - start_w) * eased;
            let frame_h = start_h + (h - start_h) * eased;
            window
                .set_size(LogicalSize::new(frame_w, frame_h))
                .map_err(|e| AppError::Invalid(format!("调整窗口尺寸失败: {e}")))?;
            if let (Some(start), Some(target)) = (old_position, target_position) {
                let frame_x = start.x as f64 + (target.x - start.x) as f64 * eased;
                let frame_y = start.y as f64 + (target.y - start.y) as f64 * eased;
                let _ = window.set_position(PhysicalPosition::new(
                    frame_x.round() as i32,
                    frame_y.round() as i32,
                ));
            }
            thread::sleep(Duration::from_millis(14));
        }
        if let Some(position) = target_position.or(old_position) {
            let _ = window.set_position(position);
        }
    } else {
        window
            .set_size(LogicalSize::new(w, h))
            .map_err(|e| AppError::Invalid(format!("调整窗口尺寸失败: {e}")))?;
    }
    if mode == WindowMode::Full && old_mode != WindowMode::Full {
        if let (Some(position), Some(source_size), Some(monitor)) =
            (old_position, old_size, monitor.as_ref())
        {
            let work = monitor.work_area();
            let target_width = (w * scale).round() as i32;
            let target_height = (h * scale).round() as i32;
            let threshold = (16.0 * scale).round() as i32;
            let target_x = aligned_axis(
                position.x,
                source_size.width as i32,
                target_width,
                work.position.x,
                work.size.width as i32,
                threshold,
            );
            let target_y = aligned_axis(
                position.y,
                source_size.height as i32,
                target_height,
                work.position.y,
                work.size.height as i32,
                threshold,
            );
            let _ = window.set_position(PhysicalPosition::new(target_x, target_y));
        }
    }
    if let Err(e) = set_setting(db, SETTING_WINDOW_MODE, mode.label()) {
        // 补偿：恢复旧模式的固定尺寸
        let (ow, oh) = old_mode.size();
        let _ = window.set_size(LogicalSize::new(ow, oh));
        let _ = window.set_resizable(false);
        return Err(e);
    }
    // 通知前端同步视图（托盘路径与命令路径统一状态源）
    let _ = app.emit(EVENT_WINDOW_STATE_CHANGED, current_state(db));
    // 以明确的目标模式保存几何（避免切换瞬间按旧模式写错 key）
    save_geometry_for(app, db, mode);
    // Resize/move events fire synchronously while switching and still observe
    // the old persisted mode. Put its pre-switch geometry back afterwards.
    if old_mode != mode {
        let old_key = match old_mode {
            WindowMode::Full => SETTING_GEOMETRY_FULL,
            WindowMode::Mini => SETTING_POS_MINI,
            WindowMode::Ball => SETTING_POS_BALL,
        };
        if let Some(saved) = source_geometry {
            if let Ok(json) = serde_json::to_string(&saved) {
                let _ = set_setting(db, old_key, &json);
            }
        }
    }
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
        return Err(e);
    }
    // 通知前端同步（Settings 开关与事件保持一致）
    let _ = app.emit(EVENT_WINDOW_STATE_CHANGED, current_state(db));
    Ok(())
}

/// Snap the floating window to a nearby work-area edge after a native drag.
/// The short interpolation avoids a visible jump while leaving normal drags
/// untouched when they are farther than the threshold.
pub fn snap_window_to_work_area(app: &tauri::AppHandle) -> Result<(), AppError> {
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .ok_or_else(|| AppError::Invalid("主窗口不存在".into()))?;
    let monitor = window
        .current_monitor()
        .map_err(|e| AppError::Invalid(format!("读取显示器失败: {e}")))?
        .ok_or_else(|| AppError::Invalid("未找到当前显示器".into()))?;
    let position = window
        .outer_position()
        .map_err(|e| AppError::Invalid(format!("读取窗口位置失败: {e}")))?;
    let size = window
        .outer_size()
        .map_err(|e| AppError::Invalid(format!("读取窗口尺寸失败: {e}")))?;
    let work = monitor.work_area();
    let threshold = (16.0 * window.scale_factor().unwrap_or(1.0)).round() as i32;
    let target_x = snapped_axis(
        position.x,
        size.width as i32,
        work.position.x,
        work.size.width as i32,
        threshold,
    );
    let target_y = snapped_axis(
        position.y,
        size.height as i32,
        work.position.y,
        work.size.height as i32,
        threshold,
    );
    if target_x == position.x && target_y == position.y {
        return Ok(());
    }
    for frame in 1..=6 {
        let progress = frame as f64 / 6.0;
        let eased = 1.0 - (1.0 - progress).powi(3);
        let x = position.x as f64 + (target_x - position.x) as f64 * eased;
        let y = position.y as f64 + (target_y - position.y) as f64 * eased;
        let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
        thread::sleep(Duration::from_millis(12));
    }
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

    // 三种模式均只允许各自的预设尺寸；Full 固定为 460×720。
    let (w, h) = state.mode.size();
    window.set_fullscreen(false)?;
    window.unmaximize()?;
    window.set_maximizable(false)?;
    window.set_resizable(false)?;
    window.set_size(LogicalSize::new(w, h))?;

    // 位置：各模式独立恢复（坐标非负才应用，越界交由系统校正）
    let pos_key = match state.mode {
        WindowMode::Full => SETTING_GEOMETRY_FULL,
        WindowMode::Mini => SETTING_POS_MINI,
        WindowMode::Ball => SETTING_POS_BALL,
    };
    if let Some(g) = load_geometry(&db, pos_key) {
        let sf = window.scale_factor().unwrap_or(1.0);
        if is_geometry_visible(&window, g, sf) {
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

#[cfg(test)]
mod tests {
    use super::{aligned_axis, rects_intersect, snapped_axis, Geometry, WindowMode};

    #[test]
    fn every_mode_has_a_fixed_size() {
        assert_eq!(WindowMode::Full.size(), (460.0, 720.0));
        assert_eq!(WindowMode::Mini.size(), (280.0, 96.0));
        assert_eq!(WindowMode::Ball.size(), (96.0, 96.0));
    }

    #[test]
    fn expansion_keeps_near_edges_aligned() {
        assert_eq!(aligned_axis(4, 96, 460, 0, 1920, 16), 0);
        assert_eq!(aligned_axis(1824, 96, 460, 0, 1920, 16), 1460);
    }

    #[test]
    fn expansion_keeps_middle_position_when_it_fits() {
        assert_eq!(aligned_axis(700, 96, 460, 0, 1920, 16), 700);
    }

    #[test]
    fn edge_snap_uses_work_area_and_negative_origins() {
        assert_eq!(snapped_axis(8, 96, 0, 1200, 16), 0);
        assert_eq!(snapped_axis(1096, 96, 0, 1200, 16), 1104);
        assert_eq!(snapped_axis(-1910, 96, -1920, 1920, 16), -1920);
        assert_eq!(snapped_axis(980, 96, 0, 1080, 16), 984);
        assert_eq!(snapped_axis(970, 96, 0, 1080, 8), 970);
    }

    #[test]
    fn negative_secondary_monitor_coordinates_are_valid() {
        assert!(rects_intersect(
            Geometry {
                x: -400.0,
                y: 100.0,
                width: 280.0,
                height: 96.0,
            },
            Geometry {
                x: -1920.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
        ));
    }

    #[test]
    fn fully_offscreen_geometry_is_rejected() {
        assert!(!rects_intersect(
            Geometry {
                x: 5000.0,
                y: 5000.0,
                width: 280.0,
                height: 96.0,
            },
            Geometry {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
        ));
    }
}
