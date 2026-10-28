//! File attribute, lock, volume and long-path helpers.
//!
//! On Windows the attribute checks call `GetFileAttributesW`, which reads the
//! attributes from the directory entry and never opens the file, so it cannot
//! trigger hydration of a cloud placeholder. Off Windows the functions return
//! the neutral answer documented on each one, so the workspace builds and
//! tests on Linux.

use std::io;
use std::path::{Path, PathBuf};

// Win32 file attribute bits (winnt.h). Kept here so the bit tests below are
// unit-testable on every OS.
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0000_0002;
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x0000_0004;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

const PLACEHOLDER_MASK: u32 =
    FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;

#[cfg_attr(not(windows), allow(dead_code))]
fn attrs_placeholder(attrs: u32) -> bool {
    attrs & PLACEHOLDER_MASK != 0
}

#[cfg_attr(not(windows), allow(dead_code))]
fn attrs_hidden_or_system(attrs: u32) -> bool {
    attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg_attr(not(windows), allow(dead_code))]
fn attrs_reparse_point(attrs: u32) -> bool {
    attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// True if `path` is a cloud-files placeholder whose data is not on disk
/// (OneDrive / Dropbox "on demand"): the RECALL_ON_DATA_ACCESS, RECALL_ON_OPEN
/// or OFFLINE attribute is set.
///
/// Reads attributes only; never opens or reads the file. Returns false if the
/// attributes cannot be read (for example the path does not exist).
/// Off Windows: always false.
pub fn is_cloud_placeholder(path: &Path) -> bool {
    #[cfg(windows)]
    {
        win::attributes(path).is_some_and(attrs_placeholder)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// True if `path` has the Windows HIDDEN or SYSTEM attribute.
///
/// Off Windows: always false (dot-files are handled in valet-core).
pub fn is_hidden_or_system(path: &Path) -> bool {
    #[cfg(windows)]
    {
        win::attributes(path).is_some_and(attrs_hidden_or_system)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// True if `path` itself is a reparse point (symlink, junction, mount point,
/// cloud placeholder with a reparse tag). Links are not followed.
///
/// Off Windows: true if `path` is a symlink.
pub fn is_reparse_point(path: &Path) -> bool {
    #[cfg(windows)]
    {
        win::attributes(path).is_some_and(attrs_reparse_point)
    }
    #[cfg(not(windows))]
    {
        std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
    }
}

/// True if another process holds `path` open in a way that blocks exclusive
/// access (Windows sharing violation, error 32, or lock violation, error 33).
///
/// The check opens the file for reading with no sharing and closes it at
/// once; it reads no data. The caller must check [`is_cloud_placeholder`]
/// first and never call this on a placeholder. As a second guard this
/// function returns false without opening anything if `path` is a
/// placeholder. Other open errors (missing file, access denied, directory)
/// also return false; the operation that follows reports them.
///
/// Off Windows: always false.
pub fn is_locked(path: &Path) -> bool {
    #[cfg(windows)]
    {
        if is_cloud_placeholder(path) {
            return false;
        }
        use std::os::windows::fs::OpenOptionsExt;
        match std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(to_long_path(path))
        {
            Ok(_file) => false,
            Err(e) => matches!(e.raw_os_error(), Some(32) | Some(33)),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// True if `a` and `b` are on the same volume, so `a` can be moved under `b`
/// with an atomic rename. Pass existing paths (the source file and the
/// destination directory).
///
/// Windows: resolves each path to its volume root with `GetVolumePathNameW`,
/// maps the root to its `\\?\Volume{GUID}\` name when the volume has one, and
/// compares case-insensitively. Off Windows: compares `st_dev`.
///
/// If env `VALET_FORCE_CROSS_VOLUME=1` is set, returns `Ok(false)` without
/// looking at the paths (tests use it to force the copy path). On error,
/// callers should take the cross-volume path.
pub fn same_volume(a: &Path, b: &Path) -> io::Result<bool> {
    if std::env::var_os("VALET_FORCE_CROSS_VOLUME").is_some_and(|v| v == "1") {
        return Ok(false);
    }
    #[cfg(windows)]
    {
        Ok(win::volume_id(a)? == win::volume_id(b)?)
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // The source is moved as itself (a link stays a link); the destination
        // directory is where the entry lands, so follow links there.
        let dev_a = std::fs::symlink_metadata(a)?.dev();
        let dev_b = std::fs::metadata(b)?.dev();
        Ok(dev_a == dev_b)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (a, b);
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

/// True if sending `path` to the trash keeps it recoverable.
///
/// Windows: the volume must be a fixed local drive (removable drives and
/// network shares have no Recycle Bin, and the shell would delete the file
/// permanently), the volume must not have "Remove files immediately when
/// deleted" (`NukeOnDelete`) set, and the `NoRecycleFiles` policy must not be
/// set. Any lookup failure returns false.
///
/// Off Windows: true (the freedesktop trash returns an error instead of
/// deleting when no trash directory is usable).
pub fn can_trash(path: &Path) -> bool {
    #[cfg(windows)]
    {
        win::can_trash(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        true
    }
}

/// Adds the `\\?\` prefix to absolute Windows paths so Win32 calls accept
/// paths longer than 260 characters. `C:\a` becomes `\\?\C:\a`,
/// `\\server\share\a` becomes `\\?\UNC\server\share\a`. Already-prefixed and
/// relative paths are returned unchanged. When the prefix is added, `/` is
/// turned into `\` and `.` / `..` components are resolved lexically, because
/// prefixed paths are passed to the file system without normalization.
///
/// Off Windows: returns the path unchanged.
pub fn to_long_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        match path.to_str() {
            Some(s) => PathBuf::from(to_long_path_str(s)),
            None => path.to_path_buf(),
        }
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

/// Splits `rest` on both separators, drops empty and `.` parts, and applies
/// `..` without popping below the first `keep` parts.
fn normalize_components(rest: &str, keep: usize) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for part in rest.split(is_sep) {
        match part {
            "" | "." => {}
            ".." => {
                if out.len() > keep {
                    out.pop();
                }
            }
            p => out.push(p),
        }
    }
    out
}

/// String core of [`to_long_path`], OS-independent so it is tested everywhere.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
pub(crate) fn to_long_path_str(s: &str) -> String {
    if s.starts_with(r"\\?\") || s.starts_with(r"\\.\") || s.starts_with(r"\??\") {
        return s.to_owned();
    }
    let mut chars = s.chars();
    let (c0, c1, c2) = (chars.next(), chars.next(), chars.next());

    // Drive-absolute: `C:\...` or `C:/...`.
    if let (Some(d), Some(':'), Some(sep)) = (c0, c1, c2)
        && d.is_ascii_alphabetic()
        && is_sep(sep)
    {
        let parts = normalize_components(&s[3..], 0);
        return format!(r"\\?\{d}:\{}", parts.join(r"\"));
    }

    // UNC: `\\server\share\...` (needs both server and share).
    if let (Some(a), Some(b)) = (c0, c1)
        && is_sep(a)
        && is_sep(b)
    {
        let rest = &s[2..];
        let mut it = rest.split(is_sep);
        let server = it.next().unwrap_or("");
        let share = it.next().unwrap_or("");
        if server.is_empty() || share.is_empty() || server == "." || server == "?" {
            return s.to_owned();
        }
        let parts = normalize_components(rest, 2);
        return format!(r"\\?\UNC\{}", parts.join(r"\"));
    }

    // Relative, drive-relative (`C:a`) or root-relative (`\a`): unchanged.
    s.to_owned()
}

/// Removes the `\\?\` prefix from a volume root returned by Win32:
/// `\\?\C:\` becomes `C:\`, `\\?\UNC\srv\share\` becomes `\\srv\share\`.
/// `\\?\Volume{...}\` and other strings are unchanged.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn strip_verbatim_str(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            return rest.to_owned();
        }
    }
    s.to_owned()
}

/// Extracts `{GUID}` from `\\?\Volume{GUID}\`.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn volume_guid(volume_name: &str) -> Option<&str> {
    let start = volume_name.find('{')?;
    let end = volume_name[start..].find('}')? + start;
    Some(&volume_name[start..=end])
}

#[cfg(windows)]
mod win {
    use super::{strip_verbatim_str, to_long_path, volume_guid};
    use std::ffi::OsStr;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows_sys::Win32::Storage::FileSystem::{
        GetDriveTypeW, GetFileAttributesW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
        INVALID_FILE_ATTRIBUTES,
    };
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RegGetValueW,
    };

    /// `DRIVE_FIXED` from winbase.h (kept local to avoid another windows-sys feature).
    const DRIVE_FIXED: u32 = 3;

    fn wide_nul(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    fn from_wide_nul(buf: &[u16]) -> String {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..len])
    }

    /// Absolute, `\\?\`-prefixed form of `path`, NUL-terminated.
    fn long_abs_wide(path: &Path) -> io::Result<Vec<u16>> {
        let abs = std::path::absolute(path)?;
        Ok(wide_nul(to_long_path(&abs).as_os_str()))
    }

    /// Attributes of `path` itself (links are not followed). Does not open the file.
    pub(super) fn attributes(path: &Path) -> Option<u32> {
        let wide = long_abs_wide(path).ok()?;
        // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
        let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
        (attrs != INVALID_FILE_ATTRIBUTES).then_some(attrs)
    }

    struct Volume {
        /// Volume root without `\\?\`, e.g. `C:\` or `\\srv\share\`.
        root: String,
        /// `\\?\Volume{GUID}\` for local volumes; None for network shares.
        name: Option<String>,
    }

    /// `GetVolumePathNameW` on one spelling of the path; returns the root as given by Win32.
    fn volume_path_name(wide: &[u16]) -> io::Result<String> {
        let mut buf = vec![0u16; wide.len().max(300) + 2];
        // SAFETY: input is NUL-terminated; `buf.len()` is the real buffer size in u16 units.
        let ok = unsafe { GetVolumePathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(from_wide_nul(&buf))
    }

    fn volume(path: &Path) -> io::Result<Volume> {
        // Never pass a relative path: Win32 resolves it against the boot volume.
        let abs = std::path::absolute(path)?;
        // Plain absolute form first; the `\\?\` form only if that fails (long paths).
        let raw = match volume_path_name(&wide_nul(abs.as_os_str())) {
            Ok(r) => r,
            Err(_) => volume_path_name(&wide_nul(to_long_path(&abs).as_os_str()))?,
        };
        let root = strip_verbatim_str(&raw);
        let root_w = wide_nul(OsStr::new(&root));
        let mut name_buf = [0u16; 64];
        // SAFETY: `root_w` is NUL-terminated and ends with `\`; buffer length matches.
        let ok = unsafe {
            GetVolumeNameForVolumeMountPointW(
                root_w.as_ptr(),
                name_buf.as_mut_ptr(),
                name_buf.len() as u32,
            )
        };
        let name = (ok != 0).then(|| from_wide_nul(&name_buf));
        Ok(Volume { root, name })
    }

    pub(super) fn volume_id(path: &Path) -> io::Result<String> {
        let v = volume(path)?;
        Ok(v.name.unwrap_or(v.root).to_lowercase())
    }

    fn reg_dword(hkey: HKEY, subkey: &str, value: &str) -> Option<u32> {
        let sk = wide_nul(OsStr::new(subkey));
        let val = wide_nul(OsStr::new(value));
        let mut data: u32 = 0;
        let mut size: u32 = std::mem::size_of::<u32>() as u32;
        // SAFETY: strings are NUL-terminated; `data`/`size` describe a 4-byte buffer.
        let rc = unsafe {
            RegGetValueW(
                hkey,
                sk.as_ptr(),
                val.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut data as *mut u32).cast(),
                &mut size,
            )
        };
        (rc == 0).then_some(data)
    }

    pub(super) fn can_trash(path: &Path) -> bool {
        let Ok(v) = volume(path) else {
            return false;
        };
        let root_w = wide_nul(OsStr::new(&v.root));
        // SAFETY: `root_w` is a NUL-terminated root path ending with `\`.
        if unsafe { GetDriveTypeW(root_w.as_ptr()) } != DRIVE_FIXED {
            return false;
        }
        const POLICY: &str = r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer";
        for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
            if reg_dword(hive, POLICY, "NoRecycleFiles").unwrap_or(0) != 0 {
                return false;
            }
        }
        if let Some(guid) = v.name.as_deref().and_then(volume_guid) {
            let key = format!(
                r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{guid}"
            );
            if reg_dword(HKEY_CURRENT_USER, &key, "NukeOnDelete").unwrap_or(0) != 0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_path_drive() {
        assert_eq!(
            to_long_path_str(r"C:\Users\a\f.txt"),
            r"\\?\C:\Users\a\f.txt"
        );
        assert_eq!(to_long_path_str(r"d:/x/y"), r"\\?\d:\x\y");
        assert_eq!(to_long_path_str(r"C:\"), r"\\?\C:\");
        assert_eq!(to_long_path_str(r"C:\a\.\b\..\c\\d\"), r"\\?\C:\a\c\d");
        assert_eq!(to_long_path_str(r"C:\..\a"), r"\\?\C:\a");
    }

    #[test]
    fn long_path_unc() {
        assert_eq!(
            to_long_path_str(r"\\server\share\dir\f"),
            r"\\?\UNC\server\share\dir\f"
        );
        assert_eq!(to_long_path_str(r"\\server\share"), r"\\?\UNC\server\share");
        assert_eq!(
            to_long_path_str(r"\\server\share\..\..\x"),
            r"\\?\UNC\server\share\x"
        );
        assert_eq!(to_long_path_str(r"\\server"), r"\\server");
    }

    #[test]
    fn long_path_already_prefixed() {
        for s in [
            r"\\?\C:\a\..\b",
            r"\\?\UNC\srv\share\x",
            r"\\.\PhysicalDrive0",
            r"\??\C:\a",
        ] {
            assert_eq!(to_long_path_str(s), s);
        }
    }

    #[test]
    fn long_path_relative() {
        for s in ["a\\b", "a/b", ".\\a", "..\\a", "C:a", "\\a\\b", "", "f.txt"] {
            assert_eq!(to_long_path_str(s), s);
        }
    }

    #[test]
    fn long_path_long_input() {
        let seg = "d".repeat(50);
        let p = format!(r"C:\{seg}\{seg}\{seg}\{seg}\{seg}\{seg}\f.txt");
        assert!(p.len() > 300);
        assert_eq!(to_long_path_str(&p), format!(r"\\?\{p}"));
    }

    #[cfg(not(windows))]
    #[test]
    fn to_long_path_is_identity_off_windows() {
        let p = Path::new("/tmp/a/../b");
        assert_eq!(to_long_path(p), p);
    }

    #[test]
    fn strip_verbatim() {
        assert_eq!(strip_verbatim_str(r"\\?\C:\"), r"C:\");
        assert_eq!(strip_verbatim_str(r"\\?\UNC\srv\share\"), r"\\srv\share\");
        assert_eq!(strip_verbatim_str(r"C:\"), r"C:\");
        let v = r"\\?\Volume{12345678-aaaa-bbbb-cccc-1234567890ab}\";
        assert_eq!(strip_verbatim_str(v), v);
    }

    #[test]
    fn guid_from_volume_name() {
        assert_eq!(
            volume_guid(r"\\?\Volume{12345678-aaaa-bbbb-cccc-1234567890ab}\"),
            Some("{12345678-aaaa-bbbb-cccc-1234567890ab}")
        );
        assert_eq!(volume_guid(r"C:\"), None);
    }

    #[test]
    fn attribute_bits() {
        assert!(attrs_placeholder(FILE_ATTRIBUTE_OFFLINE));
        assert!(attrs_placeholder(FILE_ATTRIBUTE_RECALL_ON_OPEN));
        assert!(attrs_placeholder(
            FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | 0x20
        ));
        assert!(!attrs_placeholder(0x20 | FILE_ATTRIBUTE_REPARSE_POINT));
        assert!(attrs_hidden_or_system(FILE_ATTRIBUTE_HIDDEN));
        assert!(attrs_hidden_or_system(FILE_ATTRIBUTE_SYSTEM));
        assert!(!attrs_hidden_or_system(0x20));
        assert!(attrs_reparse_point(FILE_ATTRIBUTE_REPARSE_POINT));
        assert!(!attrs_reparse_point(0x20));
    }

    #[test]
    fn same_volume_two_tempdirs() {
        // Both under the same temp root, so the same volume. Skipped if the
        // force flag is set in this process's environment.
        if std::env::var_os("VALET_FORCE_CROSS_VOLUME").is_some() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        std::fs::write(a.join("f.txt"), b"x").unwrap();
        assert!(same_volume(&a.join("f.txt"), &b).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn same_volume_missing_path_errors() {
        if std::env::var_os("VALET_FORCE_CROSS_VOLUME").is_some() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        assert!(same_volume(&root.path().join("missing"), root.path()).is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn neutral_answers_off_windows() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f");
        std::fs::write(&f, b"x").unwrap();
        assert!(!is_cloud_placeholder(&f));
        assert!(!is_hidden_or_system(&f));
        assert!(!is_locked(&f));
        assert!(!is_reparse_point(&f));
        assert!(can_trash(&f));
    }

    #[cfg(unix)]
    #[test]
    fn reparse_point_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.txt");
        std::fs::write(&target, b"x").unwrap();
        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(is_reparse_point(&link));
        assert!(!is_reparse_point(&target));
        assert!(!is_reparse_point(&dir.path().join("missing")));
    }
}
