//! Sending files to the Recycle Bin (Windows) or the freedesktop trash (Linux),
//! and restoring them from it.

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Moves `path` (file or directory) to the Recycle Bin / trash. Never deletes
/// permanently: if the volume has no usable Recycle Bin ([`crate::can_trash`]
/// is false) it returns an `Unsupported` error and leaves the file in place.
///
/// The `trash` crate runs on a short-lived thread of its own, so its COM
/// initialization on Windows cannot clash with the caller thread's COM mode,
/// and a panic inside it becomes an error.
pub fn trash_path(path: &Path) -> io::Result<()> {
    // Fails with NotFound before anything else if there is nothing to trash.
    std::fs::symlink_metadata(path)?;
    if !crate::can_trash(path) {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "volume has no usable Recycle Bin; file left in place",
        ));
    }
    let owned = path.to_path_buf();
    let handle = std::thread::Builder::new()
        .name("valet-trash".into())
        .spawn(move || trash::delete(&owned))?;
    match handle.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(io::Error::other(format!("trash failed: {e}"))),
        Err(_) => Err(io::Error::other("trash failed: worker thread panicked")),
    }
}

/// Clock slack, in seconds, when matching an item's deletion time: the bin
/// stores whole seconds.
pub const DELETED_TOLERANCE_SECS: i64 = 2;

/// Moves the most recent trash item that came from `original` back to
/// `original`. Never overwrites.
///
/// Only items deleted at or after `trashed_after` minus
/// [`DELETED_TOLERANCE_SECS`] are considered. Locations are compared as given
/// and with the parent folder of `original` canonicalized (the `trash` crate
/// records canonical parents); on Windows the comparison ignores case, `/` vs
/// `\` and the `\\?\` prefix.
///
/// Errors:
/// - `AlreadyExists`: something exists at `original`; the item stays in the bin.
/// - `NotFound`: no matching item in the bin.
/// - `Unsupported`: the platform has no listable trash (macOS).
/// - other: listing or restoring failed. If another process creates
///   `original` while the restore runs, the item ends up next to it under a
///   `.valet-restore-*` name (given in the error) instead of replacing it.
///
/// The item is restored under a fresh temporary name in the original folder
/// and then renamed with [`crate::rename_no_replace`], because the Windows
/// shell restore used by the `trash` crate replaces a file that appears at the
/// target between the crate's existence check and the move.
///
/// Like [`trash_path`], the `trash` crate runs on a short-lived thread of its
/// own (COM on Windows).
pub fn restore_from_trash(original: &Path, trashed_after: SystemTime) -> io::Result<()> {
    if std::fs::symlink_metadata(crate::to_long_path(original)).is_ok() {
        return Err(occupied());
    }
    let original = original.to_path_buf();
    let handle = std::thread::Builder::new()
        .name("valet-restore".into())
        .spawn(move || restore_impl(&original, trashed_after))?;
    handle
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("restore failed: worker thread panicked")))
}

fn occupied() -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        "original location is occupied; item left in the bin",
    )
}

#[cfg(any(
    windows,
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
))]
fn restore_impl(original: &Path, trashed_after: SystemTime) -> io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use trash::os_limited;

    let after_secs = match trashed_after.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        Err(_) => 0,
    };
    let min_deleted = after_secs.saturating_sub(DELETED_TOLERANCE_SECS);
    let wanted = [path_key(original), path_key(&canonical_parent(original))];

    let items =
        os_limited::list().map_err(|e| io::Error::other(format!("trash list failed: {e}")))?;
    let item = items
        .into_iter()
        .filter(|i| i.time_deleted >= min_deleted)
        .filter(|i| wanted.contains(&path_key(&i.original_path())))
        .max_by_key(|i| i.time_deleted)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no matching item in the bin"))?;

    let Some(parent) = original.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "original path has no parent folder",
        ));
    };
    std::fs::create_dir_all(crate::to_long_path(parent))?;

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let temp_name = format!(
        ".valet-restore-{}-{}-{}",
        std::process::id(),
        nanos,
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let temp = parent.join(&temp_name);
    if std::fs::symlink_metadata(crate::to_long_path(&temp)).is_ok() {
        return Err(io::Error::other("temporary restore name is taken"));
    }
    // Checked again right before the bin is touched.
    if std::fs::symlink_metadata(crate::to_long_path(original)).is_ok() {
        return Err(occupied());
    }
    let staged = trash::TrashItem {
        name: temp_name.into(),
        ..item
    };
    os_limited::restore_all([staged])
        .map_err(|e| io::Error::other(format!("trash restore failed: {e}")))?;

    crate::rename_no_replace(&temp, original).map_err(|e| {
        io::Error::other(format!(
            "restored as {} because the final rename failed: {e}",
            temp.display()
        ))
    })
}

#[cfg(not(any(
    windows,
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
)))]
fn restore_impl(original: &Path, trashed_after: SystemTime) -> io::Result<()> {
    let _ = (original, trashed_after);
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "listing the trash is not supported on this platform",
    ))
}

/// `original` with its parent folder canonicalized. Unchanged if the parent
/// cannot be canonicalized.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn canonical_parent(original: &Path) -> PathBuf {
    match (original.parent(), original.file_name()) {
        (Some(parent), Some(name)) => match std::fs::canonicalize(parent) {
            Ok(p) => p.join(name),
            Err(_) => original.to_path_buf(),
        },
        _ => original.to_path_buf(),
    }
}

/// Comparison key for trash locations. Windows: separators unified, `\\?\`
/// (and `\\?\UNC\`) removed, trailing separators dropped, lowercased.
/// Elsewhere: the path as is.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn path_key(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        let s = s.replace('/', "\\");
        let s = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{rest}")
        } else if let Some(rest) = s.strip_prefix(r"\\?\") {
            rest.to_string()
        } else {
            s
        };
        s.trim_end_matches('\\').to_lowercase()
    } else {
        s.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn trash_file_leaves_dir() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("valet-trash-test.txt");
        std::fs::write(&f, b"x").unwrap();
        trash_path(&f).unwrap();
        assert!(std::fs::symlink_metadata(&f).is_err());
    }

    #[test]
    fn trash_missing_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let err = trash_path(&dir.path().join("missing")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    fn dir_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn restore_round_trip_keeps_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("valet-restore-roundtrip.bin");
        let bytes: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
        std::fs::write(&f, &bytes).unwrap();
        let before = SystemTime::now();
        trash_path(&f).unwrap();
        assert!(std::fs::symlink_metadata(&f).is_err());

        restore_from_trash(&f, before).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), bytes);
        // No staging file left behind.
        assert_eq!(dir_names(dir.path()), ["valet-restore-roundtrip.bin"]);
    }

    #[test]
    fn restore_onto_occupied_path_is_already_exists_and_keeps_item() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("valet-restore-occupied.txt");
        std::fs::write(&f, b"original").unwrap();
        let before = SystemTime::now();
        trash_path(&f).unwrap();
        std::fs::write(&f, b"newcomer").unwrap();

        let err = restore_from_trash(&f, before).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read(&f).unwrap(), b"newcomer");

        // Free the name: the item is still in the bin and restorable.
        let moved = dir.path().join("moved-away.txt");
        std::fs::rename(&f, &moved).unwrap();
        restore_from_trash(&f, before).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"original");
        assert_eq!(std::fs::read(&moved).unwrap(), b"newcomer");
        assert_eq!(
            dir_names(dir.path()),
            ["moved-away.txt", "valet-restore-occupied.txt"]
        );
    }

    #[test]
    fn restore_never_trashed_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("valet-restore-never-trashed.txt");
        let err = restore_from_trash(&f, SystemTime::UNIX_EPOCH).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn restore_ignores_items_trashed_before_the_cutoff() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("valet-restore-cutoff.txt");
        std::fs::write(&f, b"x").unwrap();
        trash_path(&f).unwrap();

        let later = SystemTime::now() + Duration::from_secs(60);
        let err = restore_from_trash(&f, later).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);

        // Still in the bin; restore it so the test leaves nothing behind.
        restore_from_trash(&f, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"x");
    }

    #[test]
    fn restore_picks_the_most_recent_item() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("valet-restore-twice.txt");
        std::fs::write(&f, b"first").unwrap();
        trash_path(&f).unwrap();
        // Deletion times are whole seconds.
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&f, b"second").unwrap();
        trash_path(&f).unwrap();

        restore_from_trash(&f, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"second");
        std::fs::rename(&f, dir.path().join("second.txt")).unwrap();
        restore_from_trash(&f, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"first");
    }

    #[test]
    fn restore_directory() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().join("valet-restore-dir");
        std::fs::create_dir(&d).unwrap();
        std::fs::write(d.join("inner.txt"), b"inner").unwrap();
        let before = SystemTime::now();
        trash_path(&d).unwrap();

        restore_from_trash(&d, before).unwrap();
        assert_eq!(std::fs::read(d.join("inner.txt")).unwrap(), b"inner");
    }

    #[cfg(windows)]
    #[test]
    fn path_key_ignores_case_prefix_and_separators() {
        assert_eq!(
            path_key(Path::new(r"\\?\C:\Users\Ana\Doc.TXT")),
            path_key(Path::new("c:/users/ana/doc.txt"))
        );
        assert_eq!(
            path_key(Path::new(r"\\?\UNC\srv\share\a")),
            path_key(Path::new(r"\\SRV\Share\A\"))
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn path_key_is_exact_off_windows() {
        assert_ne!(path_key(Path::new("/a/B")), path_key(Path::new("/a/b")));
    }
}
