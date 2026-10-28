//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

use tauri::{Manager, Runtime};
use valet_core::platform::Platform;
use valet_platform::WatchCancel;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) _db_path: PathBuf,
    /// OS services for the planner and executor (`valet_platform::SystemPlatform`).
    pub(crate) platform: Arc<dyn Platform>,
    /// Running file watcher, if one started. Taken and cancelled on quit.
    pub(crate) watcher: Arc<Mutex<Option<WatchCancel>>>,
}

impl AppState {
    pub(crate) fn new(db_path: PathBuf) -> Self {
        Self {
            paused: Arc::new(AtomicBool::new(false)),
            _db_path: db_path,
            platform: Arc::new(valet_platform::SystemPlatform),
            watcher: Arc::new(Mutex::new(None)),
        }
    }

    /// Stops the file watcher if one is running. Safe to call more than once.
    pub(crate) fn stop_watcher(&self) {
        let handle = self
            .watcher
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(handle) = handle {
            handle.cancel();
        }
    }
}

/// The one folder for `valet.sqlite3`, `config.json` and `app_settings.json`.
/// Tauri derives it from the bundle identifier, which must equal
/// `valet_core::config::APP_IDENTIFIER` (checked by a test below).
pub(crate) fn config_dir<R: Runtime, M: Manager<R>>(app: &M) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config directory: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_never_replaces_existing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        let state = AppState::new(tmp.path().join("valet.sqlite3"));

        let err = state.platform.rename_no_replace(&a, &b).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&a).unwrap(), b"a");
        assert_eq!(std::fs::read(&b).unwrap(), b"b");

        let c = tmp.path().join("c.txt");
        state.platform.rename_no_replace(&a, &c).unwrap();
        assert_eq!(std::fs::read(&c).unwrap(), b"a");
    }

    #[test]
    fn stop_watcher_without_watcher_is_a_no_op() {
        let state = AppState::new(PathBuf::from("valet.sqlite3"));
        state.stop_watcher();
        state.stop_watcher();
        assert!(state.watcher.lock().unwrap().is_none());
    }

    #[test]
    fn tauri_identifier_matches_core_identifier() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(
            conf["identifier"].as_str(),
            Some(valet_core::config::APP_IDENTIFIER)
        );
    }
}
