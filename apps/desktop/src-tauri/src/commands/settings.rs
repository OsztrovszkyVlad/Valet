//! Settings commands: watched-folder config (`config.json`) and app settings (`app_settings.json`).

use tauri::Manager;

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct Config {
    pub(crate) inbox_paths: Vec<String>,
    pause_watchers: bool,
    #[serde(default = "default_quarantine_days")]
    quarantine_retention_days: u32,
}

fn default_quarantine_days() -> u32 {
    30
}

#[tauri::command]
pub(crate) async fn get_config(app: tauri::AppHandle) -> Result<Config, String> {
    let app_cfg_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))?;
    let cfg_path = app_cfg_dir.join("config.json");

    if !cfg_path.exists() {
        // Return default config
        return Ok(Config {
            inbox_paths: vec![],
            pause_watchers: false,
            quarantine_retention_days: 30,
        });
    }

    let contents = std::fs::read_to_string(&cfg_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    serde_json::from_str(&contents).map_err(|e| format!("Failed to parse config JSON: {}", e))
}

#[tauri::command]
pub(crate) async fn save_config(config: Config, app: tauri::AppHandle) -> Result<Config, String> {
    log::info!(
        "Saving config with {} inbox paths",
        config.inbox_paths.len()
    );

    // Validate paths exist
    for path in &config.inbox_paths {
        let path_buf = std::path::PathBuf::from(path);
        if !path_buf.exists() {
            return Err(format!("Path does not exist: {}", path));
        }
    }

    let app_cfg_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))?;

    // Ensure config directory exists
    std::fs::create_dir_all(&app_cfg_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    let cfg_path = app_cfg_dir.join("config.json");
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    std::fs::write(&cfg_path, json).map_err(|e| format!("Failed to write config file: {}", e))?;

    log::info!("Config saved successfully to {:?}", cfg_path);
    Ok(config)
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct AppSettings {
    // Appearance
    theme: String,
    compact_mode: bool,

    // Notifications
    pub(crate) notifications_enabled: bool,
    notification_types: NotificationTypes,

    // System Integration
    auto_start: bool,
    minimize_to_tray: bool,
    close_to_tray: bool,
    show_context_menu: bool,

    // File Operations
    confirm_operations: bool,
    create_backups: bool,
    backup_location: String,
    max_backup_age_days: u32,

    // Performance
    pub(crate) max_concurrent_operations: u32,
    file_size_limit_mb: u32,
    enable_file_watching: bool,
    watch_interval_ms: u32,

    // Advanced
    log_level: String,
    max_log_files: u32,
    enable_telemetry: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NotificationTypes {
    file_operations: bool,
    errors: bool,
    daily_summary: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            compact_mode: false,
            notifications_enabled: true,
            notification_types: NotificationTypes {
                file_operations: true,
                errors: true,
                daily_summary: false,
            },
            auto_start: false,
            minimize_to_tray: true,
            close_to_tray: false,
            show_context_menu: true,
            confirm_operations: false,
            create_backups: true,
            backup_location: String::new(),
            max_backup_age_days: 30,
            max_concurrent_operations: 5,
            file_size_limit_mb: 1024,
            enable_file_watching: true,
            watch_interval_ms: 500,
            log_level: "info".to_string(),
            max_log_files: 10,
            enable_telemetry: false,
        }
    }
}

#[tauri::command]
pub(crate) async fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let app_cfg_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))?;
    let settings_path = app_cfg_dir.join("app_settings.json");

    if !settings_path.exists() {
        // Return default settings
        return Ok(AppSettings::default());
    }

    let contents = std::fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings file: {}", e))?;

    serde_json::from_str(&contents).map_err(|e| format!("Failed to parse settings JSON: {}", e))
}

#[tauri::command]
pub(crate) async fn save_app_settings(
    settings: AppSettings,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    log::info!("Saving app settings");

    let app_cfg_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))?;

    // Ensure config directory exists
    std::fs::create_dir_all(&app_cfg_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    let settings_path = app_cfg_dir.join("app_settings.json");
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    std::fs::write(&settings_path, json)
        .map_err(|e| format!("Failed to write settings file: {}", e))?;

    log::info!("App settings saved successfully to {:?}", settings_path);
    Ok(settings)
}
