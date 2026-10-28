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

/// Clock slack, in seconds, when matching an item's deletion time. The `trash`
/// crate reports deletion times as whole seconds since the Unix epoch (UTC),
/// truncated: on Windows from the bin's FILETIME, on Linux from the
/// `.trashinfo` local time converted to UTC.
pub const DELETED_TOLERANCE_SECS: i64 = 2;

/// Moves the most recent trash item that came from `original` back to
/// `original`. Never overwrites.
///
/// Only items deleted at or after `trashed_after` minus
/// [`DELETED_TOLERANCE_SECS`] are considered. An item matches when its file
/// name equals the name of `original` and its parent folder is the same
/// folder as the parent of `original`. Folders are compared as given, in long
/// form (Windows `GetLongPathNameW`, so `C:\Users\RUNNER~1` equals
/// `C:\Users\runneradmin`) and canonicalized (the `trash` crate records
/// canonical parents). On Windows the comparison ignores case, `/` vs `\` and
/// the `\\?\` prefix, and the original path of an item is read from the bin's
/// `$I` info file, because the name the shell lists hides known extensions
/// when Explorer is set to hide them.
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

    let Some(parent) = original.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "original path has no parent folder",
        ));
    };
    let Some(wanted_name) = original.file_name().map(name_key) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "original path has no file name",
        ));
    };
    let wanted_parent = location_keys(parent);

    let after_secs = match trashed_after.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        Err(_) => 0,
    };
    let min_deleted = after_secs.saturating_sub(DELETED_TOLERANCE_SECS);

    let items =
        os_limited::list().map_err(|e| io::Error::other(format!("trash list failed: {e}")))?;
    let item = items
        .into_iter()
        .filter(|i| i.time_deleted >= min_deleted)
        .filter(|i| {
            item_origins(i).iter().any(|origin| {
                origin.file_name().map(name_key).as_ref() == Some(&wanted_name)
                    && origin
                        .parent()
                        .is_some_and(|p| location_keys(p).iter().any(|k| wanted_parent.contains(k)))
            })
        })
        .max_by_key(|i| i.time_deleted)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no matching item in the bin"))?;

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

/// Paths an item may have come from.
///
/// Windows: the path recorded in the item's `$I` info file. If that file
/// cannot be read, the crate's `original_parent` joined with its `name`, and
/// with `name` plus the extension of the `$R` data file (the listed name
/// lacks the extension when Explorer hides known extensions).
#[cfg(windows)]
fn item_origins(item: &trash::TrashItem) -> Vec<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let data_file = Path::new(&item.id);
    let info_file = data_file
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix("$R"))
        .map(|rest| data_file.with_file_name(format!("$I{rest}")));
    if let Some(info_file) = info_file
        && let Ok(bytes) = std::fs::read(crate::to_long_path(&info_file))
        && let Some(wide) = parse_info_file(&bytes)
    {
        return vec![PathBuf::from(OsString::from_wide(&wide))];
    }

    let mut origins = vec![item.original_path()];
    if let Some(ext) = data_file.extension() {
        let mut name = item.name.clone();
        name.push(".");
        name.push(ext);
        origins.push(item.original_parent.join(name));
    }
    origins
}

/// Paths an item may have come from: the crate's `original_parent` joined
/// with its `name`.
#[cfg(all(
    unix,
    not(target_os = "macos"),
    not(target_os = "ios"),
    not(target_os = "android")
))]
fn item_origins(item: &trash::TrashItem) -> Vec<PathBuf> {
    vec![item.original_path()]
}

/// Original path (UTF-16, no terminating NUL) stored in a Recycle Bin `$I`
/// info file. Layout: version (u64 LE), size (u64), deletion FILETIME (u64),
/// then for version 1 (Vista to 8.1) a fixed 260-unit NUL-padded path, for
/// version 2 (Windows 10+) a u32 length in UTF-16 units (NUL included) and
/// the path. `None` for any other layout.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn parse_info_file(bytes: &[u8]) -> Option<Vec<u16>> {
    const HEADER: usize = 24;
    const V1_UNITS: usize = 260;
    const MAX_UNITS: usize = 32_768;
    let version = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    let raw = match version {
        1 => bytes.get(HEADER..HEADER + 2 * V1_UNITS)?,
        2 => {
            let units = u32::from_le_bytes(bytes.get(HEADER..HEADER + 4)?.try_into().ok()?);
            let units = usize::try_from(units).ok().filter(|&n| n <= MAX_UNITS)?;
            bytes.get(HEADER + 4..HEADER + 4 + 2 * units)?
        }
        _ => return None,
    };
    let wide: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .take_while(|&u| u != 0)
        .collect();
    (!wide.is_empty()).then_some(wide)
}

/// Comparison keys for a folder: as given, in long form (Windows
/// `GetLongPathNameW`, only for existing folders) and canonicalized (only for
/// existing folders). See [`path_key`].
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn location_keys(dir: &Path) -> Vec<String> {
    let mut keys = vec![path_key(dir)];
    #[cfg(windows)]
    if let Some(long) = long_path_name(dir) {
        keys.push(path_key(&long));
    }
    if let Ok(canonical) = std::fs::canonicalize(dir) {
        keys.push(path_key(&canonical));
    }
    keys.sort();
    keys.dedup();
    keys
}

/// Comparison key for a file name: lowercased on Windows, as is elsewhere.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn name_key(name: &std::ffi::OsStr) -> String {
    let s = name.to_string_lossy();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.into_owned()
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

/// `path` with every 8.3 short component expanded (`GetLongPathNameW` on the
/// absolute `\\?\` form; the result keeps the prefix). `None` if the path
/// does not exist or the call fails.
#[cfg(windows)]
fn long_path_name(path: &Path) -> Option<PathBuf> {
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;
    // SAFETY: `input` is NUL-terminated; the output buffer length is passed.
    win_path_call(path, |input, out, len| unsafe {
        GetLongPathNameW(input, out, len)
    })
}

/// Runs a Win32 call with the `GetLongPathNameW` contract (returns the
/// length written, or the buffer size needed including the NUL, or 0 on
/// failure) on the absolute `\\?\` form of `path`.
#[cfg(windows)]
fn win_path_call(path: &Path, call: impl Fn(*const u16, *mut u16, u32) -> u32) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let abs = std::path::absolute(path).ok()?;
    let input: Vec<u16> = crate::to_long_path(&abs)
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut buf: Vec<u16> = vec![0; 512];
    // The path can change between calls; retry a few times.
    for _ in 0..4 {
        let cap = u32::try_from(buf.len()).ok()?;
        let n = call(input.as_ptr(), buf.as_mut_ptr(), cap) as usize;
        if n == 0 {
            return None;
        }
        if n < buf.len() {
            buf.truncate(n);
            return Some(PathBuf::from(OsString::from_wide(&buf)));
        }
        buf.resize(n, 0);
    }
    None
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
        let before = SystemTime::now();
        trash_path(&f).unwrap();
        assert!(std::fs::symlink_metadata(&f).is_err());

        // Leave nothing behind in the bin.
        restore_from_trash(&f, before).unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"x");
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

    /// `$I` info file bytes: version, size 7, FILETIME, then the path.
    fn info_file(version: u64, path: &str) -> Vec<u8> {
        let wide: Vec<u16> = path.encode_utf16().collect();
        let mut b = Vec::new();
        b.extend_from_slice(&version.to_le_bytes());
        b.extend_from_slice(&7u64.to_le_bytes());
        b.extend_from_slice(&133_000_000_000_000_000u64.to_le_bytes());
        match version {
            1 => {
                let mut fixed = wide.clone();
                fixed.resize(260, 0);
                fixed
                    .iter()
                    .for_each(|u| b.extend_from_slice(&u.to_le_bytes()));
            }
            _ => {
                b.extend_from_slice(&(wide.len() as u32 + 1).to_le_bytes());
                wide.iter()
                    .for_each(|u| b.extend_from_slice(&u.to_le_bytes()));
                b.extend_from_slice(&0u16.to_le_bytes());
            }
        }
        b
    }

    #[test]
    fn parse_info_file_versions_1_and_2() {
        let path = r"C:\Users\Ana\Documente\Factură.txt";
        let wide: Vec<u16> = path.encode_utf16().collect();
        assert_eq!(parse_info_file(&info_file(1, path)), Some(wide.clone()));
        assert_eq!(parse_info_file(&info_file(2, path)), Some(wide));
    }

    #[test]
    fn parse_info_file_rejects_bad_input() {
        let path = r"C:\a\b.txt";
        assert_eq!(parse_info_file(&info_file(3, path)), None);
        assert_eq!(parse_info_file(&[]), None);
        assert_eq!(parse_info_file(&info_file(2, "")), None);
        let v2 = info_file(2, path);
        assert_eq!(parse_info_file(&v2[..v2.len() - 4]), None);
        let v1 = info_file(1, path);
        assert_eq!(parse_info_file(&v1[..100]), None);
        let mut huge = info_file(2, path);
        huge[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_info_file(&huge), None);
    }

    /// The `trash` crate reports deletion times in whole seconds since the
    /// Unix epoch (UTC), which is what `restore_from_trash` compares against.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn bin_reports_deletion_time_in_unix_seconds() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("valet-deletion-time.txt");
        std::fs::write(&f, b"t").unwrap();
        let secs =
            |t: SystemTime| t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() as i64;
        let before = SystemTime::now();
        trash_path(&f).unwrap();
        let after = SystemTime::now();

        let wanted = location_keys(dir.path());
        let times: Vec<i64> = trash::os_limited::list()
            .unwrap()
            .iter()
            .filter(|i| {
                item_origins(i).iter().any(|o| {
                    o.file_name().map(name_key).as_deref() == Some("valet-deletion-time.txt")
                        && o.parent()
                            .is_some_and(|p| location_keys(p).iter().any(|k| wanted.contains(k)))
                })
            })
            .map(|i| i.time_deleted)
            .collect();
        restore_from_trash(&f, before).unwrap();

        assert_eq!(times.len(), 1, "items found: {times:?}");
        let (lo, hi) = (secs(before) - 1, secs(after) + 1);
        assert!(
            (lo..=hi).contains(&times[0]),
            "time_deleted {} outside {lo}..={hi}",
            times[0]
        );
    }

    /// `path` with every component in 8.3 short form, if the volume has
    /// short names (`GetShortPathNameW`; the result keeps the `\\?\` prefix).
    #[cfg(windows)]
    fn short_path_name(path: &Path) -> Option<PathBuf> {
        use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
        // SAFETY: `input` is NUL-terminated; the output buffer length is passed.
        win_path_call(path, |input, out, len| unsafe {
            GetShortPathNameW(input, out, len)
        })
    }

    /// Strips `\\?\` so the path looks like what users and `%TEMP%` give.
    #[cfg(windows)]
    fn plain(path: PathBuf) -> PathBuf {
        let s = path.to_string_lossy().into_owned();
        PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s))
    }

    #[cfg(windows)]
    #[test]
    fn restore_matches_short_and_long_folder_forms() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("Valet Long Folder Name");
        std::fs::create_dir(&sub).unwrap();
        let long = plain(long_path_name(&sub).unwrap());
        let short = plain(short_path_name(&sub).unwrap());
        eprintln!(
            "long form: {}\nshort form: {}",
            long.display(),
            short.display()
        );
        if path_key(&long) == path_key(&short) {
            // 8.3 names are off on this volume: the test still runs, but
            // both forms are the same. GitHub's runners have them on.
            assert!(
                std::env::var_os("GITHUB_ACTIONS").is_none(),
                "expected 8.3 short names on the runner's temp volume"
            );
        }

        // Trashed through the short form, restored through the long form.
        let name = "Short Then Long.txt";
        std::fs::write(short.join(name), b"short").unwrap();
        let before = SystemTime::now();
        trash_path(&short.join(name)).unwrap();
        assert!(std::fs::symlink_metadata(long.join(name)).is_err());
        restore_from_trash(&long.join(name), before).unwrap();
        assert_eq!(std::fs::read(long.join(name)).unwrap(), b"short");

        // Trashed through the long form, restored through the short form.
        let name = "Long Then Short.txt";
        std::fs::write(long.join(name), b"long").unwrap();
        let before = SystemTime::now();
        trash_path(&long.join(name)).unwrap();
        assert!(std::fs::symlink_metadata(short.join(name)).is_err());
        restore_from_trash(&short.join(name), before).unwrap();
        assert_eq!(std::fs::read(short.join(name)).unwrap(), b"long");

        assert_eq!(
            dir_names(&long),
            ["Long Then Short.txt", "Short Then Long.txt"]
        );
    }

    #[cfg(windows)]
    #[test]
    fn long_path_name_expands_short_components() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("Another Long Folder Name");
        std::fs::create_dir(&sub).unwrap();
        let short = short_path_name(&sub).unwrap();
        assert_eq!(
            path_key(&long_path_name(&short).unwrap()),
            path_key(&long_path_name(&sub).unwrap())
        );
        assert!(path_key(&long_path_name(&short).unwrap()).ends_with(r"\another long folder name"));
        assert_eq!(long_path_name(&sub.join("missing")), None);
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
