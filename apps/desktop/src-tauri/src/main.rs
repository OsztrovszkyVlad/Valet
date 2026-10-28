#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use log::LevelFilter;
use tauri::menu::Menu;
use tauri::Manager;
use tauri_plugin_log::Builder;

mod commands;
mod setup;
mod state;

fn main() {
    // DEBUG lines may contain full paths; keep them out of release logs.
    let log_level = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    tauri::Builder::default()
        // Must stay the first plugin: its setup takes the instance lock while
        // `Builder::build` initializes plugins, before `setup::setup` (and with
        // it journal recovery) runs. A second process exits inside that call.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(Builder::default().level(log_level).build())
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
            commands::operations::list_batches,
            commands::operations::get_batch,
            commands::operations::undo_batch,
            commands::platform::configure_auto_start,
            commands::platform::show_in_explorer,
            commands::platform::send_notification
        ])
        .menu(Menu::new) // optional
        .setup(setup::setup)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
