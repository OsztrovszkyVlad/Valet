//! Shared application state managed by Tauri.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc};

use tauri::{Manager, Runtime};
use valet_core::platform::Platform;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) paused: Arc<AtomicBool>,
    pub(crate) _db_path: PathBuf,
}

// Temporary until T3b wires valet_platform
/// Placeholder [`Platform`]: reports no hidden, locked, placeholder or reparse
/// files and refuses to trash anything.
pub(crate) struct StdPlatform;

impl Platform for StdPlatform {
    fn trash(&self, _path: &Path) -> io::Result<()> {
        // Nothing removes files yet.
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "trash is not available yet",
        ))
    }
    fn can_trash(&self, _path: &Path) -> bool {
        false
    }
    fn is_cloud_placeholder(&self, _path: &Path) -> bool {
        false
    }
    fn is_locked(&self, _path: &Path) -> bool {
        false
    }
    fn is_hidden_or_system(&self, _path: &Path) -> bool {
        false
    }
    fn is_reparse_point(&self, _path: &Path) -> bool {
        false
    }
    fn same_volume(&self, _a: &Path, _b: &Path) -> io::Result<bool> {
        Ok(true)
    }
    fn rename_no_replace(&self, from: &Path, to: &Path) -> io::Result<()> {
        // Check-then-act; replaced by the atomic valet_platform version in T3b.
        if to.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "destination exists",
            ));
        }
        std::fs::rename(from, to)
    }
    fn os_path(&self, path: &Path) -> PathBuf {
        path.to_path_buf()
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
    fn std_platform_never_trashes_and_never_replaces() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        let p = StdPlatform;

        assert!(!p.can_trash(&a));
        assert_eq!(p.trash(&a).unwrap_err().kind(), io::ErrorKind::Unsupported);
        assert!(a.exists());

        let err = p.rename_no_replace(&a, &b).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&b).unwrap(), b"b");

        let c = tmp.path().join("c.txt");
        p.rename_no_replace(&a, &c).unwrap();
        assert_eq!(std::fs::read(&c).unwrap(), b"a");

        assert!(!p.is_cloud_placeholder(&c));
        assert!(!p.is_locked(&c));
        assert!(!p.is_hidden_or_system(&c));
        assert!(!p.is_reparse_point(&c));
        assert!(p.same_volume(&b, &c).unwrap());
        assert_eq!(p.os_path(&c), c);
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
