//! [`SystemPlatform`]: the real [`valet_core::platform::Platform`] used by the app.

use std::io;
use std::path::{Path, PathBuf};

use valet_core::platform::Platform;

/// Delegates each [`Platform`] method to the matching function of this crate.
/// Off Windows those functions give the neutral answers documented on them.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPlatform;

impl Platform for SystemPlatform {
    /// [`crate::trash_path`]: refuses with `Unsupported` when [`crate::can_trash`] is false.
    fn trash(&self, path: &Path) -> io::Result<()> {
        crate::trash_path(path)
    }

    fn can_trash(&self, path: &Path) -> bool {
        crate::can_trash(path)
    }

    fn is_cloud_placeholder(&self, path: &Path) -> bool {
        crate::is_cloud_placeholder(path)
    }

    fn is_locked(&self, path: &Path) -> bool {
        crate::is_locked(path)
    }

    fn is_hidden_or_system(&self, path: &Path) -> bool {
        crate::is_hidden_or_system(path)
    }

    fn is_reparse_point(&self, path: &Path) -> bool {
        crate::is_reparse_point(path)
    }

    fn same_volume(&self, a: &Path, b: &Path) -> io::Result<bool> {
        crate::same_volume(a, b)
    }

    fn rename_no_replace(&self, from: &Path, to: &Path) -> io::Result<()> {
        crate::rename_no_replace(from, to)
    }

    fn os_path(&self, path: &Path) -> PathBuf {
        crate::to_long_path(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegates_rename_and_trash() {
        let dir = tempfile::tempdir().unwrap();
        let p: &dyn Platform = &SystemPlatform;
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();

        let err = p.rename_no_replace(&a, &b).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&a).unwrap(), b"a");
        assert_eq!(std::fs::read(&b).unwrap(), b"b");

        let c = dir.path().join("c.txt");
        p.rename_no_replace(&a, &c).unwrap();
        assert_eq!(std::fs::read(&c).unwrap(), b"a");
        assert!(std::fs::symlink_metadata(&a).is_err());

        assert!(!p.is_cloud_placeholder(&c));
        assert!(!p.is_locked(&c));
        assert!(!p.is_hidden_or_system(&c));
        assert!(!p.is_reparse_point(&c));
        assert_eq!(p.os_path(&c), crate::to_long_path(&c));
        assert_eq!(
            p.trash(&dir.path().join("missing")).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }
}
