//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

use tauri::{Manager, Runtime};
use valet_core::planner::Plan;
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
    /// Held while a batch is executed or undone, so batches never overlap.
    pub(crate) file_ops: Arc<tauri::async_runtime::Mutex<()>>,
    /// The latest plan `dry_run` showed, with its id. `execute_operations`
    /// runs only this plan, and only when given the same id.
    pub(crate) preview: Arc<Mutex<Option<(String, Plan)>>>,
}

/// Error string `execute_operations` returns when there is no previewed plan
/// with the given id. The UI asks the user to preview again.
pub(crate) const PREVIEW_REQUIRED: &str = "preview_required";

impl AppState {
    pub(crate) fn new(db_path: PathBuf) -> Self {
        Self {
            paused: Arc::new(AtomicBool::new(false)),
            _db_path: db_path,
            platform: Arc::new(valet_platform::SystemPlatform),
            watcher: Arc::new(Mutex::new(None)),
            file_ops: Arc::new(tauri::async_runtime::Mutex::new(())),
            preview: Arc::new(Mutex::new(None)),
        }
    }

    /// Stores `plan` under `plan_id` as the latest preview, replacing any
    /// earlier one.
    pub(crate) fn store_preview(&self, plan_id: String, plan: Plan) {
        *self
            .preview
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((plan_id, plan));
    }

    /// Removes and returns the stored preview if its id is `plan_id`.
    /// Otherwise leaves it in place and returns `PREVIEW_REQUIRED`.
    pub(crate) fn take_preview(&self, plan_id: &str) -> Result<Plan, String> {
        let mut slot = self
            .preview
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match slot.take() {
            Some((id, plan)) if id == plan_id => Ok(plan),
            other => {
                *slot = other;
                Err(PREVIEW_REQUIRED.to_string())
            }
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
    fn take_preview_without_stored_plan_requires_preview() {
        let state = AppState::new(PathBuf::from("valet.sqlite3"));
        assert_eq!(state.take_preview("any").unwrap_err(), PREVIEW_REQUIRED);
    }

    #[test]
    fn take_preview_with_wrong_id_keeps_the_stored_plan() {
        let state = AppState::new(PathBuf::from("valet.sqlite3"));
        state.store_preview("p-1".to_string(), Plan::default());
        assert_eq!(state.take_preview("other").unwrap_err(), PREVIEW_REQUIRED);
        assert_eq!(state.take_preview("p-1").unwrap(), Plan::default());
    }

    #[test]
    fn take_preview_is_single_use() {
        let state = AppState::new(PathBuf::from("valet.sqlite3"));
        state.store_preview("p-1".to_string(), Plan::default());
        state.take_preview("p-1").unwrap();
        assert_eq!(state.take_preview("p-1").unwrap_err(), PREVIEW_REQUIRED);
    }

    #[test]
    fn newer_preview_replaces_older_one() {
        let state = AppState::new(PathBuf::from("valet.sqlite3"));
        state.store_preview("old".to_string(), Plan::default());
        state.store_preview("new".to_string(), Plan::default());
        assert_eq!(state.take_preview("old").unwrap_err(), PREVIEW_REQUIRED);
        state.take_preview("new").unwrap();
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
