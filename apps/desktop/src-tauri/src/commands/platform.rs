//! Platform commands: notifications, auto-start, Explorer, the two
//! external pages Valet may open (`open_external`).

use std::path::Path;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::i18n;
use crate::state::AppState;

/// Error of `configure_auto_start` in the Microsoft Store build (Windows
/// manages its startup entry).
pub(crate) const ERR_AUTOSTART_PACKAGED: &str = "autostart.packaged";
/// Error of `open_external` for a target that is not set or not known.
pub(crate) const ERR_OPEN_UNAVAILABLE: &str = "open.unavailable";
/// Windows Settings, Apps > Startup.
pub(crate) const STARTUP_SETTINGS_URI: &str = "ms-settings:startupapps";

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
///
/// Error `autostart.packaged` in the Microsoft Store build.
#[tauri::command]
pub(crate) async fn configure_auto_start(enabled: bool, app: AppHandle) -> Result<(), String> {
    if app.state::<AppState>().license.packaged {
        return Err(ERR_AUTOSTART_PACKAGED.to_string());
    }
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

/// The URI for an `open_external` target: `checkout` (the `checkout.txt`
/// URL, when set) or `startup_settings`. Anything else: `open.unavailable`.
pub(crate) fn external_uri(target: &str, checkout_url: Option<&str>) -> Result<String, String> {
    match target {
        "checkout" => checkout_url
            .map(str::to_owned)
            .ok_or_else(|| ERR_OPEN_UNAVAILABLE.to_string()),
        "startup_settings" => Ok(STARTUP_SETTINGS_URI.to_string()),
        _ => Err(ERR_OPEN_UNAVAILABLE.to_string()),
    }
}

/// Hands a fixed URI to the shell (default browser, Settings app).
#[cfg(windows)]
fn open_uri(uri: &str) -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg(uri)
        .spawn()
        .map(|_| ())
        .map_err(|e| {
            log::warn!("Failed to open an external page: {e}");
            ERR_OPEN_UNAVAILABLE.to_string()
        })
}

#[cfg(not(windows))]
fn open_uri(_uri: &str) -> Result<(), String> {
    Err(ERR_OPEN_UNAVAILABLE.to_string())
}

/// Opens the checkout page or the Windows startup apps settings. Only these
/// two; error `open.unavailable` for anything else or when checkout is not
/// set.
#[tauri::command]
pub(crate) async fn open_external(target: String, app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let uri = external_uri(&target, state.license.checkout_url.as_deref())?;
    open_uri(&uri)?;
    log::info!("Opened external page: {target}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_checkout_and_startup_settings_can_be_opened() {
        assert_eq!(
            external_uri("startup_settings", None).unwrap(),
            "ms-settings:startupapps"
        );
        assert_eq!(
            external_uri("checkout", Some("https://valet.example/buy")).unwrap(),
            "https://valet.example/buy"
        );
        assert_eq!(
            external_uri("checkout", None).unwrap_err(),
            ERR_OPEN_UNAVAILABLE
        );
        for t in ["", "https://evil.example", "file:///C:/x", "Checkout"] {
            assert_eq!(
                external_uri(t, Some("https://valet.example/buy")).unwrap_err(),
                ERR_OPEN_UNAVAILABLE,
                "{t}"
            );
        }
    }

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
