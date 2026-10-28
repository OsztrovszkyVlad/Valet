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
            commands::operations::execute_operations_bulk,
            commands::settings::get_config,
            commands::settings::save_config,
            commands::operations::get_statistics,
            commands::operations::clear_operation_history,
            commands::settings::get_app_settings,
            commands::settings::save_app_settings,
            commands::platform::send_notification,
            commands::platform::configure_auto_start,
            commands::platform::show_in_explorer,
            commands::platform::install_context_menu_integration,
            commands::platform::uninstall_context_menu_integration,
            commands::rules::get_rule_templates,
            commands::rules::create_rule_from_template,
            commands::rules::export_rules,
            commands::rules::import_rules,
            commands::rules::validate_rule,
            commands::rules::get_rule_analytics,
            commands::backup::create_backup,
            commands::backup::get_backups,
            commands::backup::recover_files,
            commands::backup::cleanup_old_backups,
            commands::backup::get_backup_stats,
            commands::backup::get_backup_config,
            commands::backup::save_backup_config,
            commands::backup::verify_backup_integrity,
            commands::diagnostics::run_system_health_check,
            commands::diagnostics::run_performance_analysis,
            commands::diagnostics::run_system_tests,
            commands::diagnostics::run_security_audit,
            commands::diagnostics::optimize_system_performance,
            commands::diagnostics::generate_system_report
        ])
        .menu(Menu::new) // optional
        .setup(setup::setup)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
