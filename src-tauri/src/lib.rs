// AI API Monitor - Rust 后端入口
mod commands;
mod db;
mod providers;
mod settings;
mod storage;
mod window_mode;

use crate::db::Db;
use crate::providers::ProviderManager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};

/// 主窗口标签名（与 tauri.conf.json 一致）。
pub const MAIN_WINDOW: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 初始化 SQLite 数据库（app data 目录）
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = Db::open(&data_dir.join("ai-api-monitor.db"))
                .expect("failed to open database");
            app.manage(db);
            app.manage(ProviderManager::new());

            setup_tray(app)?;
            setup_close_to_tray(app)?;

            // 启动时恢复窗口模式与置顶设置（V0.2）
            window_mode::restore_window_state(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::add_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::supported_provider_types,
            commands::refresh_provider,
            commands::refresh_all,
            commands::get_refresh_settings,
            commands::set_refresh_settings,
            commands::set_window_mode,
            commands::set_always_on_top,
            commands::get_window_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 系统托盘：菜单（模式切换/显示/隐藏/退出）+ 左键单击切换可见性（V0.2）。
fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let mode_full = MenuItem::with_id(app, "mode_full", "完整模式", true, None::<&str>)?;
    let mode_mini = MenuItem::with_id(app, "mode_mini", "Mini 窗口", true, None::<&str>)?;
    let mode_ball = MenuItem::with_id(app, "mode_ball", "小球模式", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&mode_full, &mode_mini, &mode_ball, &separator, &show, &hide, &quit],
    )?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("app icon").clone())
        .tooltip("AI API Monitor")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "mode_full" => switch_window_mode(app, crate::window_mode::WindowMode::Full),
            "mode_mini" => switch_window_mode(app, crate::window_mode::WindowMode::Mini),
            "mode_ball" => switch_window_mode(app, crate::window_mode::WindowMode::Ball),
            "show" => toggle_main_window(app, true),
            "hide" => toggle_main_window(app, false),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                let visible = app
                    .get_webview_window(MAIN_WINDOW)
                    .and_then(|w| w.is_visible().ok())
                    .unwrap_or(false);
                toggle_main_window(app, !visible);
            }
        })
        .build(app)?;
    Ok(())
}

/// 显示或隐藏主窗口。
fn toggle_main_window(app: &tauri::AppHandle, show: bool) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        if show {
            let _ = window.show();
            let _ = window.set_focus();
        } else {
            let _ = window.hide();
        }
    }
}

/// 托盘菜单切换窗口模式（复用 window_mode::apply_mode 并持久化）。
fn switch_window_mode(app: &tauri::AppHandle, mode: crate::window_mode::WindowMode) {
    let db = app.state::<Db>();
    let _ = crate::window_mode::apply_mode(app, &db, mode);
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 关闭按钮行为：隐藏到托盘而非退出（V0.2 桌面工具惯例）。
fn setup_close_to_tray(app: &tauri::App) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let win = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = win.hide();
            }
        });
    }
    Ok(())
}
