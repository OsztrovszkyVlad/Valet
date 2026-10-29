//! In-app updates (PLAN 4.7, docs/RELEASE.md).
//!
//! The updater plugin runs only when the build config has `plugins.updater`
//! (release builds merge `tauri.updater.conf.json`, generated in CI with the
//! public key). Dev builds and dry runs have none: no plugin, no network,
//! and the update commands return `update.notConfigured`.
//!
//! The plugin stops at the first endpoint that answers, so every check sets
//! the one endpoint of the user's channel (`effective_update_channel`).
//! These checks and the download are the only network calls Valet makes.
//!
//! Automatic checks run 30 s after startup and then once every 24 h. They
//! only tell the UI (event `update-available`) and show a notification;
//! installing always needs the user's click (`install_update`).

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tauri::async_runtime::Mutex as AsyncMutex;
use tauri::{AppHandle, Emitter, Manager, Runtime, Url};
use tauri_plugin_updater::{Error as UpdaterError, Update, UpdaterExt};
use tokio::sync::OwnedMutexGuard;

use crate::commands::platform::notify;
use crate::commands::settings::{effective_update_channel, load_app_settings};
use crate::i18n::{self, Lang};
use crate::state::{config_dir, AppState};

/// Stable channel: the release GitHub marks "Latest".
pub(crate) const STABLE_ENDPOINT: &str =
    "https://github.com/OsztrovszkyVlad/Valet/releases/latest/download/latest.json";
/// Beta channel: kept by the release workflow on the `updater-channel`
/// pre-release; the newest published version, pre-release or stable.
pub(crate) const BETA_ENDPOINT: &str =
    "https://github.com/OsztrovszkyVlad/Valet/releases/download/updater-channel/latest-beta.json";

pub(crate) const ERR_NOT_CONFIGURED: &str = "update.notConfigured";
pub(crate) const ERR_NETWORK: &str = "update.network";
pub(crate) const ERR_BUSY: &str = "update.busy";
pub(crate) const ERR_NO_UPDATE: &str = "update.noUpdate";
pub(crate) const ERR_FAILED: &str = "update.failed";

/// Event with `{ version }` when an automatic check finds an update.
pub(crate) const EVENT_AVAILABLE: &str = "update-available";
/// Event with `{ downloaded, total }` (bytes) while `install_update` downloads.
pub(crate) const EVENT_PROGRESS: &str = "update-progress";

const STARTUP_DELAY: Duration = Duration::from_secs(30);
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// How often the loop looks at the clock; the wall clock decides when 24 h
/// passed, so time in sleep or hibernation counts.
const TICK: Duration = Duration::from_secs(60 * 60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Progress events at most once per this many bytes (and at the end).
const PROGRESS_STEP: u64 = 256 * 1024;

/// True when the app config has an updater section (`plugins.updater`).
pub(crate) fn configured(config: &tauri::Config) -> bool {
    config.plugins.0.contains_key("updater")
}

/// The endpoint for an update channel; anything but `"beta"` is stable.
pub(crate) fn endpoint_for(channel: &str) -> &'static str {
    match channel {
        "beta" => BETA_ENDPOINT,
        _ => STABLE_ENDPOINT,
    }
}

/// Managed only when the updater is configured. Holds the update the last
/// check found, which `install_update` installs.
#[derive(Default)]
pub(crate) struct UpdateSlot {
    found: Mutex<Option<Update>>,
    /// Version the last notification was shown for, so a 24 h check does
    /// not repeat it.
    notified: Mutex<Option<String>>,
}

impl UpdateSlot {
    fn set(&self, update: Option<Update>) {
        *self.found.lock().unwrap_or_else(|p| p.into_inner()) = update;
    }

    fn get(&self) -> Option<Update> {
        self.found.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// True the first time it is called for `version`.
    fn first_notice(&self, version: &str) -> bool {
        let mut last = self.notified.lock().unwrap_or_else(|p| p.into_inner());
        if last.as_deref() == Some(version) {
            return false;
        }
        *last = Some(version.to_string());
        true
    }
}

/// `check_for_update` result. Optional fields are left out when unset.
#[derive(Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct UpdateInfo {
    pub(crate) available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) notes: Option<String>,
    /// RFC 3339, as published in the release manifest (`pub_date`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) date: Option<String>,
}

impl UpdateInfo {
    fn from_update(update: Option<&Update>) -> Self {
        match update {
            None => Self::default(),
            Some(u) => Self {
                available: true,
                version: Some(u.version.clone()),
                notes: u.body.clone().filter(|n| !n.trim().is_empty()),
                date: u
                    .raw_json
                    .get("pub_date")
                    .and_then(|d| d.as_str())
                    .map(str::to_owned),
            },
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AvailablePayload {
    pub(crate) version: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProgressPayload {
    pub(crate) downloaded: u64,
    pub(crate) total: Option<u64>,
}

/// The slot, or `update.notConfigured` when the updater is not configured.
fn require_slot(slot: Option<&UpdateSlot>) -> Result<&UpdateSlot, String> {
    slot.ok_or_else(|| ERR_NOT_CONFIGURED.to_string())
}

/// Takes `AppState.file_ops` without waiting: `update.busy` while a batch
/// is executed or undone. Holding the guard keeps new batches from starting.
pub(crate) fn try_hold_file_ops(
    file_ops: &Arc<AsyncMutex<()>>,
) -> Result<OwnedMutexGuard<()>, String> {
    file_ops
        .clone()
        .try_lock_owned()
        .map_err(|_| ERR_BUSY.to_string())
}

/// Error code for a failed check or download.
fn error_code(e: &UpdaterError) -> &'static str {
    match e {
        UpdaterError::Reqwest(_) | UpdaterError::Network(_) => ERR_NETWORK,
        _ => ERR_FAILED,
    }
}

/// The channel from the settings file.
fn current_channel<R: Runtime>(app: &AppHandle<R>) -> String {
    let settings = config_dir(app)
        .map(|dir| load_app_settings(&dir))
        .unwrap_or_default();
    effective_update_channel(&settings).to_owned()
}

/// Asks the endpoint of `channel`. `Ok(None)`: no newer version. An
/// endpoint without a manifest (404 until the first release of a channel)
/// counts as no update.
async fn check_channel<R: Runtime>(
    app: &AppHandle<R>,
    channel: &str,
) -> Result<Option<Update>, String> {
    let url = Url::parse(endpoint_for(channel)).map_err(|_| ERR_FAILED.to_string())?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .and_then(|b| b.timeout(REQUEST_TIMEOUT).build())
        .map_err(|e| {
            log::error!("Updater setup failed: {e}");
            ERR_FAILED.to_string()
        })?;
    match updater.check().await {
        Ok(update) => Ok(update),
        Err(UpdaterError::ReleaseNotFound) => {
            log::warn!("No update manifest on the {channel} channel");
            Ok(None)
        }
        Err(e) => {
            log::warn!("Update check failed: {e}");
            Err(error_code(&e).to_string())
        }
    }
}

/// Checks the channel from the settings and stores the result for
/// `install_update`.
async fn check_and_store<R: Runtime>(
    app: &AppHandle<R>,
    slot: &UpdateSlot,
) -> Result<UpdateInfo, String> {
    let channel = current_channel(app);
    let update = check_channel(app, &channel).await?;
    match &update {
        Some(u) => log::info!("Update available on the {channel} channel: {}", u.version),
        None => log::info!("No update on the {channel} channel"),
    }
    let info = UpdateInfo::from_update(update.as_ref());
    slot.set(update);
    Ok(info)
}

/// The running app version (the one the updater compares against).
#[tauri::command]
pub(crate) fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Checks for an update on the user's channel. Errors: `update.notConfigured`,
/// `update.network`, `update.failed`.
#[tauri::command]
pub(crate) async fn check_for_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let slot = app.try_state::<UpdateSlot>();
    let slot = require_slot(slot.as_deref())?;
    check_and_store(&app, slot).await
}

/// Downloads, verifies and installs the update the last check found, then
/// restarts Valet. On Windows the plugin starts the NSIS installer (passive
/// mode, restarts the app) and exits this process.
///
/// Errors: `update.notConfigured`; `update.busy` while a batch runs (the
/// file-operations lock stays held until the process exits, so no batch
/// starts during the install); `update.noUpdate` when no check found one;
/// `update.network`, `update.failed`.
#[tauri::command]
pub(crate) async fn install_update(app: AppHandle) -> Result<(), String> {
    let slot = app.try_state::<UpdateSlot>();
    let slot = require_slot(slot.as_deref())?;
    let state = app.state::<AppState>();
    let _file_ops = try_hold_file_ops(&state.file_ops)?;
    let update = slot.get().ok_or_else(|| ERR_NO_UPDATE.to_string())?;
    log::info!("Downloading update {}", update.version);

    let mut downloaded: u64 = 0;
    let mut reported: u64 = 0;
    let progress_app = app.clone();
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if downloaded - reported >= PROGRESS_STEP || Some(downloaded) == total {
                    reported = downloaded;
                    let _ =
                        progress_app.emit(EVENT_PROGRESS, ProgressPayload { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| {
            log::warn!("Update download failed: {e}");
            error_code(&e).to_string()
        })?;

    // The watcher keeps running: its batches wait for `file_ops`, which
    // this command holds until the process exits.
    log::info!("Installing update {}", update.version);
    log::logger().flush();
    // Windows: does not return on success.
    if let Err(e) = update.install(bytes) {
        log::error!("Update install failed: {e}");
        return Err(ERR_FAILED.to_string());
    }
    log::info!("Update installed, restarting");
    log::logger().flush();
    app.restart()
}

/// Starts the automatic checks: 30 s after startup, then every 24 h. Call
/// after journal recovery, only when the updater is configured.
pub(crate) fn start_auto_check<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        let mut last: Option<SystemTime> = None;
        loop {
            let now = SystemTime::now();
            if check_due(last, now) {
                last = Some(now);
                auto_check(&app).await;
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

/// True when no check ran yet or the last one is 24 h old (or the clock
/// went backwards).
pub(crate) fn check_due(last: Option<SystemTime>, now: SystemTime) -> bool {
    match last {
        None => true,
        Some(last) => now
            .duration_since(last)
            .map_or(true, |elapsed| elapsed >= CHECK_INTERVAL),
    }
}

async fn auto_check<R: Runtime>(app: &AppHandle<R>) {
    let Some(slot) = app.try_state::<UpdateSlot>() else {
        return;
    };
    let Ok(info) = check_and_store(app, &slot).await else {
        return; // logged in check_channel
    };
    let Some(version) = info.version.filter(|_| info.available) else {
        return;
    };
    let _ = app.emit(
        EVENT_AVAILABLE,
        AvailablePayload {
            version: version.clone(),
        },
    );
    if slot.first_notice(&version) {
        notify(
            app,
            i18n::text(i18n::current_lang(app)).batch_title,
            &available_body(i18n::current_lang(app), &version),
        );
    }
}

/// Notification text for an available update.
pub(crate) fn available_body(lang: Lang, version: &str) -> String {
    match lang {
        Lang::Ro => format!("Versiunea {version} este disponibilă. O poți instala din Setări."),
        Lang::En => format!("Version {version} is available. You can install it from Settings."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_by_channel() {
        assert_eq!(endpoint_for("stable"), STABLE_ENDPOINT);
        assert_eq!(endpoint_for("beta"), BETA_ENDPOINT);
        assert_eq!(endpoint_for("anything"), STABLE_ENDPOINT);
        assert!(Url::parse(STABLE_ENDPOINT).is_ok());
        assert!(Url::parse(BETA_ENDPOINT).is_ok());
    }

    #[test]
    fn endpoints_match_the_release_config_script() {
        let script = include_str!("../../../../scripts/write-updater-config.mjs");
        assert!(script.contains(STABLE_ENDPOINT));
        assert!(script.contains(BETA_ENDPOINT));
    }

    #[test]
    fn busy_while_file_ops_lock_is_held() {
        let lock = Arc::new(AsyncMutex::new(()));
        let held = lock.clone().try_lock_owned().unwrap();
        assert_eq!(try_hold_file_ops(&lock).unwrap_err(), ERR_BUSY);
        drop(held);
        let guard = try_hold_file_ops(&lock).unwrap();
        // Held by the install: a batch cannot take it.
        assert!(lock.try_lock().is_err());
        drop(guard);
        assert!(lock.try_lock().is_ok());
    }

    #[test]
    fn not_configured_without_updater_section() {
        let conf: tauri::Config = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert!(
            !configured(&conf),
            "tauri.conf.json must not have plugins.updater"
        );
        assert_eq!(
            require_slot(None).err().as_deref(),
            Some(ERR_NOT_CONFIGURED)
        );

        let mut with = serde_json::to_value(&conf).unwrap();
        with["plugins"]["updater"] = serde_json::json!({
            "pubkey": "x",
            "endpoints": [STABLE_ENDPOINT, BETA_ENDPOINT]
        });
        let with: tauri::Config = serde_json::from_value(with).unwrap();
        assert!(configured(&with));
        let slot = UpdateSlot::default();
        assert!(require_slot(Some(&slot)).is_ok());
        assert!(
            slot.get().is_none(),
            "install before a check is update.noUpdate"
        );
    }

    #[test]
    fn update_info_without_update_has_only_available() {
        let json = serde_json::to_value(UpdateInfo::from_update(None)).unwrap();
        assert_eq!(json, serde_json::json!({ "available": false }));
    }

    #[test]
    fn progress_payload_shape() {
        let json = serde_json::to_value(ProgressPayload {
            downloaded: 10,
            total: None,
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({ "downloaded": 10, "total": null }));
    }

    #[test]
    fn checks_every_24_hours() {
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert!(check_due(None, t0));
        assert!(!check_due(
            Some(t0),
            t0 + CHECK_INTERVAL - Duration::from_secs(1)
        ));
        assert!(check_due(Some(t0), t0 + CHECK_INTERVAL));
        assert!(
            check_due(Some(t0), t0 - Duration::from_secs(1)),
            "clock went back"
        );
    }

    #[test]
    fn notifies_once_per_version() {
        let slot = UpdateSlot::default();
        assert!(slot.first_notice("1.0.1"));
        assert!(!slot.first_notice("1.0.1"));
        assert!(slot.first_notice("1.0.2"));
    }

    #[test]
    fn notification_text() {
        assert_eq!(
            available_body(Lang::Ro, "1.0.1"),
            "Versiunea 1.0.1 este disponibilă. O poți instala din Setări."
        );
        assert_eq!(
            available_body(Lang::En, "1.0.1"),
            "Version 1.0.1 is available. You can install it from Settings."
        );
    }
}
