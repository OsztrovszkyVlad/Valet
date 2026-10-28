//! Platform commands: notifications, auto-start, Explorer, context menu.

use tauri::Manager;

use crate::commands::settings::AppSettings;

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
    let app_cfg_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))?;
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
                log::info!("Notification sent: {} - {}", title, message);
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
        log::info!(
            "Notification (unsupported platform): {} - {}",
            title,
            message
        );
        Ok(())
    }
}

/*
fn send_desktop_notification(title: &str, message: &str, level: NotificationLevel) -> Result<(), String> {
  #[cfg(target_os = "windows")]
  {
    use notify_rust::Notification;

    let icon = match level {
      NotificationLevel::Info => "info",
      NotificationLevel::Success => "security",
      NotificationLevel::Warning => "warning",
      NotificationLevel::Error => "error",
    };

    match Notification::new()
      .summary(title)
      .body(message)
      .icon(icon)
      .timeout(notify_rust::Timeout::Milliseconds(5000))
      .show()
    {
      Ok(_) => {
        log::info!("Desktop notification sent: {} - {}", title, message);
        Ok(())
      }
      Err(e) => {
        log::warn!("Failed to send desktop notification: {}", e);
        Ok(()) // Don't fail the operation if notification fails
      }
    }
  }

  #[cfg(not(target_os = "windows"))]
  {
    log::warn!("Desktop notifications not supported on this platform");
    Ok(())
  }
}
*/

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

#[tauri::command]
pub(crate) async fn install_context_menu_integration() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_CLASSES_ROOT);

        // Get the current executable path
        let exe_path =
            std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;

        // Install context menu for directories
        let dir_key = hklm
            .create_subkey(r"Directory\shell\ValetOrganize")
            .map_err(|e| format!("Failed to create directory context menu key: {}", e))?;

        dir_key
            .0
            .set_value("", &"Organize with Valet")
            .map_err(|e| format!("Failed to set directory menu text: {}", e))?;

        dir_key
            .0
            .set_value("Icon", &format!("{},0", exe_path.display()))
            .map_err(|e| format!("Failed to set directory menu icon: {}", e))?;

        let dir_command_key = dir_key
            .0
            .create_subkey("command")
            .map_err(|e| format!("Failed to create directory command key: {}", e))?;

        dir_command_key
            .0
            .set_value(
                "",
                &format!("\"{}\" --organize-folder \"%1\"", exe_path.display()),
            )
            .map_err(|e| format!("Failed to set directory command: {}", e))?;

        // Install context menu for files
        let file_key = hklm
            .create_subkey(r"*\shell\ValetOrganize")
            .map_err(|e| format!("Failed to create file context menu key: {}", e))?;

        file_key
            .0
            .set_value("", &"Organize with Valet")
            .map_err(|e| format!("Failed to set file menu text: {}", e))?;

        file_key
            .0
            .set_value("Icon", &format!("{},0", exe_path.display()))
            .map_err(|e| format!("Failed to set file menu icon: {}", e))?;

        let file_command_key = file_key
            .0
            .create_subkey("command")
            .map_err(|e| format!("Failed to create file command key: {}", e))?;

        file_command_key
            .0
            .set_value(
                "",
                &format!("\"{}\" --organize-file \"%1\"", exe_path.display()),
            )
            .map_err(|e| format!("Failed to set file command: {}", e))?;

        log::info!("Windows context menu integration installed successfully");
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Context menu integration is only supported on Windows".to_string())
    }
}

#[tauri::command]
pub(crate) async fn uninstall_context_menu_integration() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_CLASSES_ROOT);

        // Remove directory context menu
        if let Ok(dir_shell) = hklm.open_subkey(r"Directory\shell") {
            let _ = dir_shell.delete_subkey_all("ValetOrganize");
        }

        // Remove file context menu
        if let Ok(file_shell) = hklm.open_subkey(r"*\shell") {
            let _ = file_shell.delete_subkey_all("ValetOrganize");
        }

        log::info!("Windows context menu integration removed");
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Context menu integration is only supported on Windows".to_string())
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
