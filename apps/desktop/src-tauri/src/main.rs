#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use log::LevelFilter;
use tauri::menu::Menu;
use tauri::WindowEvent;
use tauri_plugin_log::Builder;

use commands::platform::launched_minimized;

mod auto;
mod commands;
mod i18n;
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
        // A second launch shows the running instance, unless it is the
        // auto-start entry (`--minimized`).
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !launched_minimized(&args) {
                setup::show_main_window(app);
            }
        }))
        .plugin(Builder::default().level(log_level).build())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::rules::get_rules,
            commands::rules::upsert_rule,
            commands::rules::delete_rule,
            commands::presets::list_presets,
            commands::presets::install_presets,
            commands::presets::complete_onboarding,
            commands::operations::dry_run,
            commands::operations::execute_operations,
            commands::operations::close_preview,
            commands::settings::get_config,
            commands::settings::save_config,
            commands::settings::get_app_settings,
            commands::settings::save_app_settings,
            commands::operations::list_batches,
            commands::operations::get_batch,
            commands::operations::undo_batch,
            commands::platform::configure_auto_start,
            commands::platform::get_auto_start,
            commands::platform::show_in_explorer,
            commands::platform::send_notification
        ])
        .menu(Menu::new) // optional
        // Closing the main window hides it: Valet keeps running in the tray
        // (watcher, auto-apply). The tray's quit item exits.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(setup::setup)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
