#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Instant;

use tauri::menu::Menu;
use tauri::WindowEvent;

use commands::platform::launched_minimized;

mod auto;
mod commands;
mod i18n;
mod logging;
mod setup;
mod state;
mod updater;

fn main() {
    // Startup timings in the log ("Database ready N ms after start") count
    // from here.
    let started = Instant::now();
    logging::install_panic_hook();

    let context = tauri::generate_context!();
    // Release builds merge `plugins.updater` from tauri.updater.conf.json;
    // without it the plugin would fail to start, so it is left out.
    let updater_configured = updater::configured(context.config());

    let mut builder = tauri::Builder::default()
        // Must stay the first plugin: its setup takes the instance lock while
        // `Builder::build` initializes plugins, before `setup::start_db_init`
        // (and with it journal recovery) runs. A second process exits inside
        // that call.
        // A second launch shows the running instance, unless it is the
        // auto-start entry (`--minimized`).
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !launched_minimized(&args) {
                setup::show_main_window(app);
            }
        }))
        // DEBUG lines may contain full paths; INFO by default in release
        // (`logging::set_debug_logging` follows the setting).
        .plugin(logging::plugin(&logging::log_settings()))
        .plugin(tauri_plugin_notification::init());
    if updater_configured {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .manage(updater::UpdateSlot::default());
    }

    let app = builder
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
            commands::platform::send_notification,
            updater::get_app_version,
            updater::check_for_update,
            updater::install_update
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
        .build(context)
        .expect("error while building tauri application");
    // The instance lock is held now. Open the database and run recovery on
    // the async runtime while `run` creates the main window; `setup::setup`
    // waits for both before the watcher starts.
    setup::start_db_init(&app, started);
    app.run(|_, _| {});
}
