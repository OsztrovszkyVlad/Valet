//! OS services the planner and executor need. valet-core stays OS-agnostic;
//! valet-platform implements this trait (Windows attributes, Recycle Bin, long paths).
use std::{
    io,
    path::{Path, PathBuf},
};

pub trait Platform: Send + Sync {
    /// Moves `path` to the Recycle Bin / OS trash. Never deletes permanently.
    fn trash(&self, path: &Path) -> io::Result<()>;
    /// The OS trash can take this path (Windows: fixed drive, Recycle Bin enabled for its volume).
    /// When false, `trash` must not be attempted; the file stays.
    fn can_trash(&self, path: &Path) -> bool;
    /// Unhydrated cloud placeholder (OneDrive/Dropbox). Must not open or read the file.
    fn is_cloud_placeholder(&self, path: &Path) -> bool;
    /// Another process holds the file open without sharing. Never called on placeholders.
    fn is_locked(&self, path: &Path) -> bool;
    /// Hidden or system attribute (Windows). Dot-files are handled by the planner itself.
    fn is_hidden_or_system(&self, path: &Path) -> bool;
    /// Junction, symlink or other reparse point: never traversed, never moved.
    /// Means "points somewhere else" (name-surrogate tags: symlink, junction, mount point).
    /// Cloud-file reparse points (OneDrive/Dropbox, hydrated or not) must return false here:
    /// placeholders are reported by `is_cloud_placeholder`, hydrated cloud files are normal files.
    fn is_reparse_point(&self, path: &Path) -> bool;
    /// Both existing paths are on the same volume (rename is atomic).
    fn same_volume(&self, a: &Path, b: &Path) -> io::Result<bool>;
    /// Renames `from` to `to`, failing with AlreadyExists if `to` exists. Never replaces.
    fn rename_no_replace(&self, from: &Path, to: &Path) -> io::Result<()>;
    /// Path form for std::fs calls (adds \\?\ on Windows for long paths).
    fn os_path(&self, path: &Path) -> PathBuf;
}

/// In-memory platform for tests. Not for product code.
#[cfg(any(test, feature = "test-support"))]
pub mod testing {
    use super::Platform;
    use std::{
        collections::HashSet,
        fs, io,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    /// Test double for [`Platform`].
    ///
    /// - `trash` renames the file into `trash_dir` under a unique name (nothing is deleted);
    ///   with `trash_dir == None` it fails with `Unsupported`.
    /// - placeholder / locked / hidden answers are membership in the public sets (exact path).
    /// - `is_reparse_point` is `symlink_metadata().is_symlink()`.
    /// - `rename_no_replace` checks that `to` is absent, then calls `std::fs::rename`.
    ///   That is check-then-act: another process can create `to` in between and
    ///   `std::fs::rename` may then replace it. Tests only; the real implementation
    ///   must be atomic (valet-platform).
    /// - `os_path` is the identity.
    #[derive(Debug)]
    pub struct FakePlatform {
        pub trash_dir: Option<PathBuf>,
        pub placeholders: HashSet<PathBuf>,
        pub locked: HashSet<PathBuf>,
        pub hidden: HashSet<PathBuf>,
        /// Answer of `same_volume` (default true).
        pub same_volume: bool,
        /// Answer of `can_trash` (default true).
        pub can_trash: bool,
        trash_counter: AtomicU64,
    }

    impl Default for FakePlatform {
        fn default() -> Self {
            Self {
                trash_dir: None,
                placeholders: HashSet::new(),
                locked: HashSet::new(),
                hidden: HashSet::new(),
                same_volume: true,
                can_trash: true,
                trash_counter: AtomicU64::new(0),
            }
        }
    }

    impl FakePlatform {
        /// Fake whose `trash` moves files into `trash_dir`.
        pub fn with_trash_dir(trash_dir: impl Into<PathBuf>) -> Self {
            Self {
                trash_dir: Some(trash_dir.into()),
                ..Self::default()
            }
        }
    }

    impl Platform for FakePlatform {
        fn trash(&self, path: &Path) -> io::Result<()> {
            let dir = self.trash_dir.as_ref().ok_or_else(|| {
                io::Error::new(io::ErrorKind::Unsupported, "FakePlatform has no trash_dir")
            })?;
            fs::create_dir_all(dir)?;
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
            loop {
                let n = self.trash_counter.fetch_add(1, Ordering::SeqCst);
                let mut unique = std::ffi::OsString::from(format!("{n}-"));
                unique.push(name);
                let target = dir.join(unique);
                match self.rename_no_replace(path, &target) {
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                    other => return other,
                }
            }
        }

        fn can_trash(&self, _path: &Path) -> bool {
            self.can_trash
        }

        fn is_cloud_placeholder(&self, path: &Path) -> bool {
            self.placeholders.contains(path)
        }

        fn is_locked(&self, path: &Path) -> bool {
            self.locked.contains(path)
        }

        fn is_hidden_or_system(&self, path: &Path) -> bool {
            self.hidden.contains(path)
        }

        fn is_reparse_point(&self, path: &Path) -> bool {
            fs::symlink_metadata(path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
        }

        fn same_volume(&self, _a: &Path, _b: &Path) -> io::Result<bool> {
            Ok(self.same_volume)
        }

        fn rename_no_replace(&self, from: &Path, to: &Path) -> io::Result<()> {
            if fs::symlink_metadata(to).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "destination exists",
                ));
            }
            fs::rename(from, to)
        }

        fn os_path(&self, path: &Path) -> PathBuf {
            path.to_path_buf()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn trash_moves_into_trash_dir_with_unique_names() {
            let tmp = tempfile::tempdir().unwrap();
            let trash = tmp.path().join("trash");
            let p = FakePlatform::with_trash_dir(&trash);
            let a = tmp.path().join("a").join("x.txt");
            let b = tmp.path().join("b").join("x.txt");
            for (f, body) in [(&a, "one"), (&b, "two")] {
                fs::create_dir_all(f.parent().unwrap()).unwrap();
                fs::write(f, body).unwrap();
            }
            p.trash(&a).unwrap();
            p.trash(&b).unwrap();
            assert!(!a.exists() && !b.exists());
            let mut bodies: Vec<String> = fs::read_dir(&trash)
                .unwrap()
                .map(|e| fs::read_to_string(e.unwrap().path()).unwrap())
                .collect();
            bodies.sort();
            assert_eq!(bodies, vec!["one", "two"]);
        }

        #[test]
        fn trash_without_dir_is_unsupported_and_keeps_file() {
            let tmp = tempfile::tempdir().unwrap();
            let f = tmp.path().join("x.txt");
            fs::write(&f, "x").unwrap();
            let err = FakePlatform::default().trash(&f).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::Unsupported);
            assert!(f.exists());
        }

        #[test]
        fn rename_no_replace_refuses_existing_target() {
            let tmp = tempfile::tempdir().unwrap();
            let from = tmp.path().join("from.txt");
            let to = tmp.path().join("to.txt");
            fs::write(&from, "new").unwrap();
            fs::write(&to, "old").unwrap();
            let p = FakePlatform::default();
            let err = p.rename_no_replace(&from, &to).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
            assert_eq!(fs::read_to_string(&to).unwrap(), "old");
            assert_eq!(fs::read_to_string(&from).unwrap(), "new");

            let free = tmp.path().join("free.txt");
            p.rename_no_replace(&from, &free).unwrap();
            assert_eq!(fs::read_to_string(&free).unwrap(), "new");
            assert!(!from.exists());
        }

        #[test]
        fn predicates_follow_sets_and_flags() {
            let mut p = FakePlatform::default();
            let f = PathBuf::from("/x/a.txt");
            assert!(!p.is_cloud_placeholder(&f) && !p.is_locked(&f) && !p.is_hidden_or_system(&f));
            p.placeholders.insert(f.clone());
            p.locked.insert(f.clone());
            p.hidden.insert(f.clone());
            assert!(p.is_cloud_placeholder(&f) && p.is_locked(&f) && p.is_hidden_or_system(&f));
            assert!(p.can_trash(&f));
            assert!(p.same_volume(&f, &f).unwrap());
            p.can_trash = false;
            p.same_volume = false;
            assert!(!p.can_trash(&f));
            assert!(!p.same_volume(&f, &f).unwrap());
            assert_eq!(p.os_path(&f), f);
        }

        #[cfg(unix)]
        #[test]
        fn reparse_point_is_symlink() {
            let tmp = tempfile::tempdir().unwrap();
            let real = tmp.path().join("real.txt");
            fs::write(&real, "x").unwrap();
            let link = tmp.path().join("link.txt");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            let p = FakePlatform::default();
            assert!(p.is_reparse_point(&link));
            assert!(!p.is_reparse_point(&real));
        }
    }
}
