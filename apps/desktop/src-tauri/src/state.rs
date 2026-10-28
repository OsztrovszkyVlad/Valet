//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{Manager, Runtime};
use valet_core::planner::Plan;
use valet_core::platform::Platform;
use valet_platform::WatchCancel;

use crate::auto::AutoMsg;
use crate::commands::settings::PreviewApproval;

/// Auto-apply stays paused this long after `dry_run` when the UI neither
/// executes the preview nor calls `close_preview`.
pub(crate) const PREVIEW_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Plan id, plan, and what executing it approves (the rules it contains
/// and the watched folders it planned).
pub(crate) type StoredPreview = (String, Plan, PreviewApproval);

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
    /// The latest plan `dry_run` showed, with its id, the rules (id,
    /// version) it contains and the watched folders it planned.
    /// `execute_operations` runs only this plan, and only when given the
    /// same id.
    pub(crate) preview: Arc<Mutex<Option<StoredPreview>>>,
    /// When `dry_run` last opened a preview; `None` once it was executed or
    /// closed. Auto-apply waits while a preview is open (PLAN 4.4).
    pub(crate) preview_opened: Arc<Mutex<Option<Instant>>>,
    /// Sender to the auto-apply loop (`auto::start`); `None` before it starts.
    pub(crate) auto_tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<AutoMsg>>>>,
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
            preview_opened: Arc::new(Mutex::new(None)),
            auto_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Stores `plan` under `plan_id` as the latest preview, replacing any
    /// earlier one. Executing it records `approval` (its rules under its
    /// watched folders).
    pub(crate) fn store_preview(&self, plan_id: String, plan: Plan, approval: PreviewApproval) {
        *self
            .preview
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((plan_id, plan, approval));
    }

    /// Removes and returns the stored preview (plan and its approval) if its
    /// id is `plan_id`. Otherwise leaves it in place and returns
    /// `PREVIEW_REQUIRED`.
    pub(crate) fn take_preview(&self, plan_id: &str) -> Result<(Plan, PreviewApproval), String> {
        let mut slot = self
            .preview
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match slot.take() {
            Some((id, plan, approval)) if id == plan_id => Ok((plan, approval)),
            other => {
                *slot = other;
                Err(PREVIEW_REQUIRED.to_string())
            }
        }
    }

    /// Marks a preview as open at `now` (`dry_run`).
    pub(crate) fn open_preview(&self, now: Instant) {
        *self
            .preview_opened
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(now);
    }

    /// Marks the preview as closed (`execute_operations`, `close_preview`).
    pub(crate) fn close_preview(&self) {
        *self
            .preview_opened
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }

    /// A preview is open at `now`: opened, not closed, and opened less than
    /// [`PREVIEW_TIMEOUT`] ago.
    pub(crate) fn preview_active(&self, now: Instant) -> bool {
        self.preview_opened
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some_and(|opened| now.saturating_duration_since(opened) < PREVIEW_TIMEOUT)
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
        let approval = PreviewApproval {
            rules: vec![(uuid::Uuid::from_u128(1), 3)],
            roots: vec![PathBuf::from("C:\\Users\\Ana\\Downloads")],
        };
        state.store_preview("p-1".to_string(), Plan::default(), approval.clone());
        assert_eq!(state.take_preview("other").unwrap_err(), PREVIEW_REQUIRED);
        assert_eq!(
            state.take_preview("p-1").unwrap(),
            (Plan::default(), approval)
        );
    }

    #[test]
    fn take_preview_is_single_use() {
        let state = AppState::new(PathBuf::from("valet.sqlite3"));
        state.store_preview(
            "p-1".to_string(),
            Plan::default(),
            PreviewApproval::default(),
        );
        state.take_preview("p-1").unwrap();
        assert_eq!(state.take_preview("p-1").unwrap_err(), PREVIEW_REQUIRED);
    }

    #[test]
    fn newer_preview_replaces_older_one() {
        let state = AppState::new(PathBuf::from("valet.sqlite3"));
        state.store_preview(
            "old".to_string(),
            Plan::default(),
            PreviewApproval::default(),
        );
        state.store_preview(
            "new".to_string(),
            Plan::default(),
            PreviewApproval::default(),
        );
        assert_eq!(state.take_preview("old").unwrap_err(), PREVIEW_REQUIRED);
        state.take_preview("new").unwrap();
    }

    #[test]
    fn preview_pause_ends_on_close_or_after_timeout() {
        let state = AppState::new(PathBuf::from("valet.sqlite3"));
        let t0 = Instant::now();
        assert!(!state.preview_active(t0), "no preview yet");

        state.open_preview(t0);
        assert!(state.preview_active(t0));
        assert!(state.preview_active(t0 + PREVIEW_TIMEOUT - Duration::from_secs(1)));
        assert!(!state.preview_active(t0 + PREVIEW_TIMEOUT), "timed out");

        state.open_preview(t0);
        state.close_preview();
        assert!(!state.preview_active(t0));

        // A preview opened after `now` (while a pass waited for the lock)
        // counts as open.
        state.open_preview(t0 + Duration::from_secs(5));
        assert!(state.preview_active(t0));
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
