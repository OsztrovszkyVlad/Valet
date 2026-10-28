//! Platform commands: notifications, auto-start, Explorer.

use std::path::Path;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::i18n;

/// Argument the auto-start entry passes: start hidden in the tray.
pub(crate) const MINIMIZED_ARG: &str = valet_platform::AUTOSTART_ARG;

/// True if `args` contain [`MINIMIZED_ARG`].
pub(crate) fn launched_minimized<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|a| a.as_ref() == MINIMIZED_ARG)
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// `notifications_enabled` from `app_settings.json`; true when unset.
pub(crate) fn notifications_enabled<R: Runtime, M: Manager<R>>(app: &M) -> bool {
    i18n::app_settings_json(app)
        .as_ref()
        .and_then(|v| v.get("notifications_enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// Shows a desktop notification unless notifications are turned off in
/// settings. Failures are logged, never returned: a missing notification must
/// not fail the operation that triggered it.
pub(crate) fn notify<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    if !notifications_enabled(app) {
        log::debug!("Notifications disabled, skipping notification");
        return;
    }
    match app.notification().builder().title(title).body(body).show() {
        Ok(()) => log::info!("Notification sent"),
        Err(e) => log::warn!("Failed to send notification: {e}"),
    }
}

/// The short notification after a batch, in the UI language. Nothing when
/// the batch did nothing.
#[allow(dead_code)] // Called from the batch code once auto-apply (A3) uses it.
pub(crate) fn notify_batch_done<R: Runtime>(app: &AppHandle<R>, done: usize, failed: usize) {
    if done == 0 && failed == 0 {
        return;
    }
    let lang = i18n::current_lang(app);
    notify(
        app,
        i18n::text(lang).batch_title,
        &i18n::batch_body(lang, done, failed),
    );
}

#[tauri::command]
pub(crate) async fn send_notification(
    title: String,
    message: String,
    app: AppHandle,
) -> Result<(), String> {
    log::debug!("Notification: {} - {}", title, message);
    notify(&app, &title, &message);
    Ok(())
}

/// Adds or removes the per-user auto-start entry for the running exe
/// (`valet_platform::set_autostart`, HKCU only). The entry starts Valet with
/// [`MINIMIZED_ARG`], so it opens in the tray.
#[tauri::command]
pub(crate) async fn configure_auto_start(enabled: bool) -> Result<(), String> {
    std::env::current_exe()
        .and_then(|exe| valet_platform::set_autostart(enabled, &exe))
        .map_err(|e| format!("Failed to change auto-start: {e}"))?;
    log::info!(
        "Auto-start {}",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

/// Whether the auto-start entry exists, points at the running exe and is
/// not disabled in Task Manager (`valet_platform::autostart_state`).
#[tauri::command]
pub(crate) async fn get_auto_start() -> Result<bool, String> {
    std::env::current_exe()
        .and_then(|exe| valet_platform::autostart_state(&exe))
        .map_err(|e| format!("Failed to read auto-start state: {e}"))
}

/// Error string `show_in_explorer` returns when the path does not exist.
pub(crate) const PATH_NOT_FOUND: &str = "path_not_found";

/// Opens Explorer with `path` selected (`valet_platform::reveal_in_explorer`).
#[tauri::command]
pub(crate) async fn show_in_explorer(path: String) -> Result<(), String> {
    valet_platform::reveal_in_explorer(Path::new(&path)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            PATH_NOT_FOUND.to_string()
        } else {
            format!("Failed to show in Explorer: {e}")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimized_flag_is_detected() {
        assert!(launched_minimized(["app.exe", "--minimized"]));
        assert!(!launched_minimized(["app.exe"]));
        assert!(!launched_minimized(["app.exe", "--minimized=false"]));
    }

    #[test]
    fn show_in_explorer_missing_path_is_path_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.txt");
        let err = tauri::async_runtime::block_on(show_in_explorer(
            missing.to_string_lossy().into_owned(),
        ))
        .unwrap_err();
        assert_eq!(err, PATH_NOT_FOUND);
    }
}
