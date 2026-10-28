//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};

use tauri::{Manager, Runtime};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) _db_path: PathBuf,
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
