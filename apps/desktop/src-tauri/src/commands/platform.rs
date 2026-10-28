//! Platform commands: notifications, auto-start, Explorer.

use crate::commands::settings::AppSettings;
use crate::state::config_dir;

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

// Notification functionality
#[tauri::command]
pub(crate) async fn send_notification(
    title: String,
    message: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Check if notifications are enabled in settings
    let app_cfg_dir = config_dir(&app)?;
    let settings_path = app_cfg_dir.join("app_settings.json");

    if settings_path.exists() {
        let contents = std::fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings file: {}", e))?;

        if let Ok(settings) = serde_json::from_str::<AppSettings>(&contents) {
            if !settings.notifications_enabled {
                log::debug!("Notifications disabled, skipping notification");
                return Ok(());
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        use notify_rust::Notification;

        match Notification::new()
            .summary(&title)
            .body(&message)
            .icon("file-manager") // Use a generic file manager icon
            .timeout(notify_rust::Timeout::Milliseconds(5000))
            .show()
        {
            Ok(_) => {
                log::info!("Notification sent");
                log::debug!("Notification: {} - {}", title, message);
                Ok(())
            }
            Err(e) => {
                log::warn!("Failed to send notification: {}", e);
                // Don't fail the operation if notification fails
                Ok(())
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        log::info!("Notification skipped (unsupported platform)");
        log::debug!("Notification: {} - {}", title, message);
        Ok(())
    }
}

#[tauri::command]
pub(crate) async fn configure_auto_start(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let app_path = std::env::current_exe()
            .map_err(|e| format!("Failed to get current executable path: {}", e))?;

        let app_name = "ValetFileOrganizer";

        if enabled {
            // Add to Windows startup via registry
            let status = Command::new("reg")
                .args([
                    "add",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    app_name,
                    "/t",
                    "REG_SZ",
                    "/d",
                    &format!("\"{}\"", app_path.display()),
                    "/f",
                ])
                .status()
                .map_err(|e| format!("Failed to execute registry command: {}", e))?;

            if status.success() {
                log::info!("Auto-start enabled successfully");
                Ok(())
            } else {
                Err("Failed to enable auto-start".to_string())
            }
        } else {
            // Remove from Windows startup
            let status = Command::new("reg")
                .args([
                    "delete",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v",
                    app_name,
                    "/f",
                ])
                .status()
                .map_err(|e| format!("Failed to execute registry command: {}", e))?;

            if status.success() || status.code() == Some(1) {
                // Code 1 means the key doesn't exist
                log::info!("Auto-start disabled successfully");
                Ok(())
            } else {
                Err("Failed to disable auto-start".to_string())
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        log::warn!("Auto-start configuration not supported on this platform");
        Err("Auto-start configuration not supported on this platform".to_string())
    }
}

#[tauri::command]
pub(crate) async fn show_in_explorer(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let status = Command::new("explorer")
            .args(["/select,", &path])
            .status()
            .map_err(|e| format!("Failed to open explorer: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err("Failed to open file in explorer".to_string())
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err("Show in explorer not supported on this platform".to_string())
    }
}

pub(crate) fn show_in_explorer_internal(path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let status = Command::new("explorer")
            .args(["/select,", path])
            .status()
            .map_err(|e| format!("Failed to open explorer: {}", e))?;

        if status.success() {
            Ok(())
        } else {
            Err("Failed to open file in explorer".to_string())
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err("Show in explorer not supported on this platform".to_string())
    }
}
