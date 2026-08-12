// AI API Monitor - Rust 后端入口
mod commands;
mod db;
mod providers;
mod settings;
mod storage;

use crate::db::Db;
use crate::providers::ProviderManager;
use tauri::Manager;

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
