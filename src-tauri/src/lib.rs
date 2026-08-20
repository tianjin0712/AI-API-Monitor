// AI API Monitor - Rust 后端入口
mod assets;
mod commands;
mod db;
mod platform_security;
mod providers;
mod security;
mod settings;
mod storage;
mod subprocess;
mod window_mode;

#[cfg(test)]
mod security_tests;

use crate::db::Db;
use crate::providers::ProviderManager;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// 主窗口标签名（与 tauri.conf.json 一致）。
pub const MAIN_WINDOW: &str = "main";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = tauri::Builder::default()
        .register_uri_scheme_protocol("app-resource", |context, request| {
            assets::protocol_response(context.app_handle(), request)
        })
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // 初始化 SQLite 数据库（app data 目录）
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            platform_security::harden_private_path(&data_dir, true)?;
            let log_dir = data_dir.join("logs");
            std::fs::create_dir_all(&log_dir)?;
            platform_security::harden_private_path(&log_dir, true)?;
            security::configure_log_dir(log_dir);
            let assets = assets::AssetStore::new(&data_dir)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(assets);
            let db_path = data_dir.join("ai-api-monitor.db");
            let db = Db::open(&db_path).map_err(|error| {
                std::io::Error::other(format!("无法打开应用数据库 {}: {error}", db_path.display()))
            })?;
            let recovery_notice = db.recovery_notice().map(str::to_owned);
            app.manage(db);
            app.manage(Arc::new(ProviderManager::new()));
            app.manage(commands::AlertState::default());

            if let Some(notice) = recovery_notice {
                security::safe_log("database_recovery", &notice);
                let _ = settings::set_setting(
                    app.state::<Db>().inner(),
                    settings::SETTING_DATABASE_RECOVERY_NOTICE,
                    &notice,
                );
            }

            // 启动时执行旧凭据迁移（幂等，V3）；失败数写入 settings 供前端提示
            match settings::migrate_legacy_credentials(app.state::<Db>().inner()) {
                Ok(r) => {
                    if r.failed > 0 {
                        let _ = settings::set_setting(
                            app.state::<Db>().inner(),
                            settings::SETTING_MIGRATION_LEGACY_FAILED,
                            &r.failed.to_string(),
                        );
                        security::safe_log(
                            "setup",
                            format!("{} 个旧凭据无法读取，需用户重新录入", r.failed),
                        );
                    } else {
                        // 全部迁移成功时清除历史标记，避免警告永久残留（review should-fix）
                        let _ = settings::delete_setting(
                            app.state::<Db>().inner(),
                            settings::SETTING_MIGRATION_LEGACY_FAILED,
                        );
                    }
                }
                Err(e) => security::safe_log("setup", format!("凭据迁移失败: {e}")),
            }
            let db = app.state::<Db>();
            if let Err(error) = settings::migrate_missing_key_hints(&db) {
                security::safe_log("setup", format!("Key 掩码迁移失败: {error}"));
            }
            if let Err(error) = settings::migrate_sensitive_settings(&db) {
                security::safe_log("setup", format!("敏感设置迁移失败: {error}"));
            }
            if let Err(error) = settings::ensure_privacy_defaults(&db) {
                security::safe_log("setup", format!("隐私默认值初始化失败: {error}"));
            }

            setup_tray(app)?;
            setup_close_to_tray(app)?;
            setup_geometry_persist(app)?;

            // 启动时恢复窗口模式与置顶设置（V0.2）
            window_mode::restore_window_state(app)?;
            crate::providers::codex::start_rate_limit_monitor(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_asset,
            commands::delete_asset,
            commands::read_asset,
            commands::list_providers,
            commands::add_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::supported_provider_types,
            commands::is_custom_endpoint_approved,
            commands::approve_custom_endpoint,
            commands::refresh_provider,
            commands::refresh_all,
            commands::get_codex_runtime_status,
            commands::start_codex_login,
            commands::test_custom_provider,
            commands::get_refresh_settings,
            commands::set_refresh_settings,
            commands::get_app_behavior_settings,
            commands::set_close_behavior,
            commands::set_auto_start,
            commands::set_window_mode,
            commands::set_always_on_top,
            commands::snap_window_to_work_area,
            commands::get_window_state,
            commands::get_migration_status,
            commands::get_database_recovery_notice,
            commands::get_layout,
            commands::set_layout,
            commands::get_usage_history,
            commands::get_prediction,
            commands::check_update,
            commands::install_update,
        ])
        .run(tauri::generate_context!())
    {
        report_startup_failure(&error.to_string());
    }
}

/// Production builds have no console. Preserve a redacted startup diagnostic and show a
/// clear user-facing error instead of silently exiting (for example, when a policy blocks
/// access to the user data directory).
fn report_startup_failure(error: &str) {
    let message = security::SensitiveDataFilter::redact(error);
    let fallback = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("AI API Monitor")
        .join("logs");
    if std::fs::create_dir_all(&fallback).is_ok() {
        security::configure_log_dir(fallback.clone());
        security::safe_log("startup", &message);
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        let recovery_hint = if message.to_ascii_lowercase().contains("locked")
            || message.to_ascii_lowercase().contains("busy")
        {
            "数据库当前被其他实例占用，请关闭其他 AI API Monitor 进程后重试。"
        } else {
            "原数据库已保留；请查看日志，必要时从 migration-snapshots 恢复。"
        };
        let text: Vec<u16> = format!(
            "AI API Monitor 无法安全启动。\n\n原因：{}\n{}\n\n日志：{}",
            message,
            recovery_hint,
            fallback.join("application.log").display()
        )
        .encode_utf16()
        .chain(Some(0))
        .collect();
        let title: Vec<u16> = std::ffi::OsStr::new("AI API Monitor - 启动失败")
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
    }
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
        &[
            &mode_full, &mode_mini, &mode_ball, &separator, &show, &hide, &quit,
        ],
    )?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("app icon").clone())
        .tooltip("AI API Monitor")
        .menu(&menu)
        .show_menu_on_left_click(false) // 左键=切换可见性，右键=菜单（P1 修复交互冲突）
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
        let handle = app.handle().clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let behavior = handle.state::<Db>();
                let close_behavior = crate::settings::get_setting(
                    &behavior,
                    crate::settings::SETTING_CLOSE_BEHAVIOR,
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| "minimize_to_tray".to_string());
                if close_behavior == "minimize_to_tray" {
                    api.prevent_close();
                    let _ = win.hide();
                }
            }
        });
    }
    Ok(())
}

/// 监听窗口移动/缩放（持久化几何）与聚焦（唤醒刷新事件），P2。
fn setup_geometry_persist(app: &tauri::App) -> tauri::Result<()> {
    let handle = app.handle().clone();
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        window.on_window_event(move |event| {
            match event {
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    let db = handle.state::<Db>();
                    crate::window_mode::save_current_geometry(&handle, &db);
                }
                WindowEvent::Focused(true) => {
                    // 窗口恢复聚焦（含系统唤醒后）立即通知前端刷新
                    let _ = handle.emit("app-focused", ());
                }
                _ => {}
            }
        });
    }
    Ok(())
}
