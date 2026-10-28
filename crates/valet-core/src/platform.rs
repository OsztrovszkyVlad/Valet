//! OS services the planner and executor need. valet-core stays OS-agnostic;
//! valet-platform implements this trait (Windows attributes, Recycle Bin, long paths).
use std::{
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub trait Platform: Send + Sync {
    /// Moves `path` to the Recycle Bin / OS trash. Never deletes permanently.
    fn trash(&self, path: &Path) -> io::Result<()>;
    /// Moves the most recent Recycle Bin item that came from `original` and
    /// was trashed at or after `trashed_after` (minus a clock tolerance of a
    /// few seconds) back to `original`. Never overwrites:
    /// - `AlreadyExists`: something is at `original`; the item stays in the bin.
    /// - `NotFound`: no matching item in the bin.
    fn restore_from_trash(&self, original: &Path, trashed_after: SystemTime) -> io::Result<()>;
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
        io::Write,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, SystemTime},
    };

    /// Clock slack when matching an item's trash time (as the real bin, which
    /// stores whole seconds).
    pub const RESTORE_TOLERANCE: Duration = Duration::from_secs(2);

    /// Test double for [`Platform`].
    ///
    /// - `trash` renames the file into `trash_dir` under a unique name (nothing is deleted);
    ///   with `trash_dir == None` it fails with `Unsupported`; for paths in `trash_fails`
    ///   it fails with `PermissionDenied` and leaves the file. It records the original
    ///   path and the trash time in `<trash_dir>.info/<unique name>` (on disk, so a
    ///   restore works from another process, as in the fault-injection driver).
    /// - `restore_from_trash` renames the most recent recorded item for that original
    ///   path (trashed at or after `trashed_after - RESTORE_TOLERANCE`) back with
    ///   `rename_no_replace`; `AlreadyExists` when the original path is taken,
    ///   `NotFound` when no item matches, `Unsupported` without `trash_dir`.
    ///   Info files of restored items stay (their bin entry is gone, so they no
    ///   longer match).
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
        /// Paths whose `trash` fails (exact path).
        pub trash_fails: HashSet<PathBuf>,
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
                trash_fails: HashSet::new(),
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

        /// Folder of the trash records: `<trash_dir>.info`, beside the bin.
        pub fn info_dir(&self) -> Option<PathBuf> {
            let dir = self.trash_dir.as_ref()?;
            let mut name = dir.file_name()?.to_os_string();
            name.push(".info");
            Some(dir.with_file_name(name))
        }

        fn bin(&self) -> io::Result<(&PathBuf, PathBuf)> {
            let dir = self.trash_dir.as_ref().ok_or_else(|| {
                io::Error::new(io::ErrorKind::Unsupported, "FakePlatform has no trash_dir")
            })?;
            let info = self
                .info_dir()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad trash_dir"))?;
            Ok((dir, info))
        }
    }

    /// One `<trash_dir>.info` record: `<nanos since epoch>\n<original path>`.
    fn parse_info(text: &str) -> Option<(SystemTime, PathBuf)> {
        let (nanos, path) = text.split_once('\n')?;
        let nanos: u64 = nanos.trim().parse().ok()?;
        Some((
            SystemTime::UNIX_EPOCH + Duration::from_nanos(nanos),
            PathBuf::from(path),
        ))
    }

    impl Platform for FakePlatform {
        fn trash(&self, path: &Path) -> io::Result<()> {
            if self.trash_fails.contains(path) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "FakePlatform: trash refused for this path",
                ));
            }
            let (dir, info_dir) = self.bin()?;
            fs::create_dir_all(dir)?;
            fs::create_dir_all(&info_dir)?;
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
            loop {
                let n = self.trash_counter.fetch_add(1, Ordering::SeqCst);
                let mut unique = std::ffi::OsString::from(format!("{n}-"));
                unique.push(name);
                let info = info_dir.join(&unique);
                let target = dir.join(&unique);
                if fs::symlink_metadata(&info).is_ok() || fs::symlink_metadata(&target).is_ok() {
                    continue;
                }
                // The record is written first, so a process killed in between
                // leaves a record without a bin entry (ignored), never an
                // entry without a record (as the real bin, which is one step).
                let nanos = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let mut f = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&info)?;
                write!(f, "{nanos}\n{}", path.to_string_lossy())?;
                f.sync_all()?;
                drop(f);
                match self.rename_no_replace(path, &target) {
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                    other => return other,
                }
            }
        }

        fn restore_from_trash(&self, original: &Path, trashed_after: SystemTime) -> io::Result<()> {
            if fs::symlink_metadata(original).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "original location is occupied; item left in the bin",
                ));
            }
            let (dir, info_dir) = self.bin()?;
            let cutoff = trashed_after
                .checked_sub(RESTORE_TOLERANCE)
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let mut best: Option<(SystemTime, PathBuf)> = None;
            for entry in fs::read_dir(&info_dir).into_iter().flatten() {
                let entry = entry?;
                let Some((at, from)) = fs::read_to_string(entry.path())
                    .ok()
                    .and_then(|t| parse_info(&t))
                else {
                    continue;
                };
                let item = dir.join(entry.file_name());
                if from != original || at < cutoff || fs::symlink_metadata(&item).is_err() {
                    continue;
                }
                if best.as_ref().is_none_or(|(t, _)| at > *t) {
                    best = Some((at, item));
                }
            }
            let (_, item) = best.ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "no matching item in the bin")
            })?;
            if let Some(parent) = original.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }
            self.rename_no_replace(&item, original)
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

    /// Sets the mtime of a file or directory (test fixtures).
    ///
    /// Files are opened for writing: on Windows a read-only handle cannot
    /// change times. Directories need an OS-specific open on Windows
    /// (`FILE_FLAG_BACKUP_SEMANTICS`, `FILE_WRITE_ATTRIBUTES` access), through
    /// `std` only. This lives in the test-support module, so it is never
    /// compiled into the product.
    pub fn set_mtime(path: &Path, t: SystemTime) {
        let file = if path.is_dir() {
            open_dir_for_times(path)
        } else {
            fs::OpenOptions::new().write(true).open(path)
        };
        file.and_then(|f| f.set_modified(t))
            .unwrap_or_else(|e| panic!("set_mtime {}: {e}", path.display()));
    }

    #[cfg(windows)]
    fn open_dir_for_times(path: &Path) -> io::Result<fs::File> {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_WRITE_ATTRIBUTES: u32 = 0x0100;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        fs::OpenOptions::new()
            .access_mode(FILE_WRITE_ATTRIBUTES)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
    }

    /// POSIX sets times on any descriptor the owner holds (`futimens`).
    #[cfg(not(windows))]
    fn open_dir_for_times(path: &Path) -> io::Result<fs::File> {
        fs::File::open(path)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn set_mtime_changes_files_and_directories() {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path().join("d");
            let file = dir.join("f.txt");
            fs::create_dir_all(&dir).unwrap();
            fs::write(&file, "x").unwrap();
            let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
            set_mtime(&file, t);
            set_mtime(&dir, t);
            assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), t);
            assert_eq!(fs::metadata(&dir).unwrap().modified().unwrap(), t);
            assert_eq!(fs::read(&file).unwrap(), b"x", "content untouched");
        }

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
        fn restore_brings_back_the_latest_item_and_never_overwrites() {
            let tmp = tempfile::tempdir().unwrap();
            let p = FakePlatform::with_trash_dir(tmp.path().join("trash"));
            let f = tmp.path().join("in").join("x.txt");
            let before = SystemTime::now();
            for body in ["first", "second"] {
                fs::create_dir_all(f.parent().unwrap()).unwrap();
                fs::write(&f, body).unwrap();
                p.trash(&f).unwrap();
                std::thread::sleep(Duration::from_millis(5));
            }
            // Occupied: refused, both items stay in the bin.
            fs::write(&f, "newcomer").unwrap();
            let err = p.restore_from_trash(&f, before).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
            assert_eq!(fs::read_to_string(&f).unwrap(), "newcomer");
            assert_eq!(fs::read_dir(tmp.path().join("trash")).unwrap().count(), 2);

            fs::rename(&f, tmp.path().join("moved.txt")).unwrap();
            // The folder may be gone too: restore recreates it.
            fs::rename(f.parent().unwrap(), tmp.path().join("in-old")).unwrap();
            p.restore_from_trash(&f, before).unwrap();
            assert_eq!(fs::read_to_string(&f).unwrap(), "second");
            fs::rename(&f, tmp.path().join("second.txt")).unwrap();
            p.restore_from_trash(&f, before).unwrap();
            assert_eq!(fs::read_to_string(&f).unwrap(), "first");
            fs::rename(&f, tmp.path().join("first.txt")).unwrap();
            let err = p.restore_from_trash(&f, before).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::NotFound);
        }

        #[test]
        fn restore_ignores_items_trashed_before_the_cutoff_and_other_paths() {
            let tmp = tempfile::tempdir().unwrap();
            let p = FakePlatform::with_trash_dir(tmp.path().join("trash"));
            let f = tmp.path().join("x.txt");
            fs::write(&f, "x").unwrap();
            p.trash(&f).unwrap();
            let later = SystemTime::now() + RESTORE_TOLERANCE + Duration::from_secs(5);
            assert_eq!(
                p.restore_from_trash(&f, later).unwrap_err().kind(),
                io::ErrorKind::NotFound
            );
            let other = tmp.path().join("y.txt");
            assert_eq!(
                p.restore_from_trash(&other, SystemTime::UNIX_EPOCH)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::NotFound
            );
            // A second platform value (another process) sees the same records.
            let again = FakePlatform::with_trash_dir(tmp.path().join("trash"));
            again
                .restore_from_trash(&f, SystemTime::UNIX_EPOCH)
                .unwrap();
            assert_eq!(fs::read_to_string(&f).unwrap(), "x");
            // Trashing again after a restore reuses no record.
            again.trash(&f).unwrap();
            p.restore_from_trash(&f, SystemTime::UNIX_EPOCH).unwrap();
            assert_eq!(fs::read_to_string(&f).unwrap(), "x");
        }

        #[test]
        fn restore_directory_and_without_bin() {
            let tmp = tempfile::tempdir().unwrap();
            let p = FakePlatform::with_trash_dir(tmp.path().join("trash"));
            let d = tmp.path().join("empty");
            fs::create_dir(&d).unwrap();
            p.trash(&d).unwrap();
            assert!(!d.exists());
            p.restore_from_trash(&d, SystemTime::UNIX_EPOCH).unwrap();
            assert!(d.is_dir());
            let err = FakePlatform::default()
                .restore_from_trash(&tmp.path().join("none"), SystemTime::UNIX_EPOCH)
                .unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::Unsupported);
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
        fn trash_fails_only_for_listed_paths() {
            let tmp = tempfile::tempdir().unwrap();
            let mut p = FakePlatform::with_trash_dir(tmp.path().join("trash"));
            let keep = tmp.path().join("keep.txt");
            let go = tmp.path().join("go.txt");
            fs::write(&keep, "k").unwrap();
            fs::write(&go, "g").unwrap();
            p.trash_fails.insert(keep.clone());
            let err = p.trash(&keep).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
            assert!(keep.exists());
            p.trash(&go).unwrap();
            assert!(!go.exists());
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
