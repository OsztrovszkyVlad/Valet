#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use log::LevelFilter;
use tauri::menu::Menu;
use tauri_plugin_log::Builder;

mod commands;
mod setup;
mod state;

fn main() {
    tauri::Builder::default()
        .plugin(Builder::default().level(LevelFilter::Debug).build())
        .invoke_handler(tauri::generate_handler![
            commands::rules::get_rules,
            commands::rules::upsert_rule,
            commands::operations::dry_run,
            commands::operations::execute_operations,
            commands::settings::get_config,
            commands::settings::save_config,
            commands::settings::get_app_settings,
            commands::settings::save_app_settings,
            commands::operations::get_statistics,
            commands::operations::clear_operation_history,
            commands::platform::configure_auto_start,
            commands::platform::show_in_explorer,
            commands::platform::send_notification
        ])
        .menu(Menu::new) // optional
        .setup(setup::setup)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
