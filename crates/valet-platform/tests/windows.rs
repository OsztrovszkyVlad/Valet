//! Windows-only checks against the real file system (run on windows-latest CI).
#![cfg(windows)]

use std::ffi::OsStr;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use valet_platform::{
    autostart_entry_state, can_trash, is_cloud_placeholder, is_hidden_or_system, is_locked,
    is_reparse_point, rename_no_replace, restore_from_trash, same_volume, set_autostart_entry,
    to_long_path, trash_path,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_SYSTEM,
    SetFileAttributesW,
};

fn wide(p: &Path) -> Vec<u16> {
    OsStr::new(p)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn set_attrs(path: &Path, attrs: u32) {
    let w = wide(&to_long_path(path));
    // SAFETY: `w` is a NUL-terminated UTF-16 path.
    let ok = unsafe { SetFileAttributesW(w.as_ptr(), attrs) };
    assert!(
        ok != 0,
        "SetFileAttributesW: {}",
        std::io::Error::last_os_error()
    );
}

fn file_in(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, b"valet").unwrap();
    p
}

/// A directory under `base` whose path is longer than 300 characters.
fn deep_dir(base: &Path) -> PathBuf {
    let mut p = base.to_path_buf();
    let seg = "valet_long_path_segment_0123456789_abcdefghijklmnopqrstuvwxyz";
    while p.as_os_str().len() < 300 {
        p.push(seg);
    }
    std::fs::create_dir_all(to_long_path(&p)).unwrap();
    p
}

#[test]
fn hidden_and_system_attributes() {
    let dir = tempfile::tempdir().unwrap();
    let f = file_in(dir.path(), "h.txt");
    assert!(!is_hidden_or_system(&f));
    set_attrs(&f, FILE_ATTRIBUTE_HIDDEN);
    assert!(is_hidden_or_system(&f));
    set_attrs(&f, FILE_ATTRIBUTE_SYSTEM);
    assert!(is_hidden_or_system(&f));
    set_attrs(&f, FILE_ATTRIBUTE_NORMAL);
    assert!(!is_hidden_or_system(&f));
}

#[test]
fn offline_attribute_is_placeholder() {
    let dir = tempfile::tempdir().unwrap();
    let f = file_in(dir.path(), "cloud.txt");
    assert!(!is_cloud_placeholder(&f));
    set_attrs(&f, FILE_ATTRIBUTE_OFFLINE);
    assert!(is_cloud_placeholder(&f));
    // The lock probe must not open a placeholder; it answers false.
    assert!(!is_locked(&f));
    set_attrs(&f, FILE_ATTRIBUTE_NORMAL);
    assert!(!is_cloud_placeholder(&f));
    assert!(!is_cloud_placeholder(&dir.path().join("missing")));
}

#[test]
fn exclusive_handle_means_locked() {
    let dir = tempfile::tempdir().unwrap();
    let f = file_in(dir.path(), "locked.txt");
    assert!(!is_locked(&f));
    let held = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&f)
        .unwrap();
    assert!(is_locked(&f));
    drop(held);
    assert!(!is_locked(&f));
    assert!(!is_locked(&dir.path().join("missing")));
}

#[test]
fn symlink_is_reparse_point() {
    let dir = tempfile::tempdir().unwrap();
    let target = file_in(dir.path(), "target.txt");
    let link = dir.path().join("link.txt");
    // Creating symlinks needs admin or developer mode; skip if unavailable.
    if std::os::windows::fs::symlink_file(&target, &link).is_err() {
        eprintln!("symlink not permitted here; skipping");
        return;
    }
    assert!(is_reparse_point(&link));
    assert!(!is_reparse_point(&target));
}

#[test]
fn junction_is_reparse_point() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target_dir");
    std::fs::create_dir(&target).unwrap();
    let junction = dir.path().join("junction");
    // Junctions need no privilege, but skip if cmd/mklink is unavailable.
    let made = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(&junction)
        .arg(&target)
        .output()
        .is_ok_and(|o| o.status.success());
    if !made {
        eprintln!("mklink /J failed here; skipping");
        return;
    }
    assert!(is_reparse_point(&junction));
    assert!(!is_reparse_point(&target));
}

#[test]
fn plain_file_and_dir_are_not_reparse_points() {
    let dir = tempfile::tempdir().unwrap();
    let f = file_in(dir.path(), "plain.txt");
    let d = dir.path().join("plain_dir");
    std::fs::create_dir(&d).unwrap();
    assert!(!is_reparse_point(&f));
    assert!(!is_reparse_point(&d));
    assert!(!is_reparse_point(dir.path()));
    assert!(!is_reparse_point(&dir.path().join("missing")));
}

#[test]
fn long_path_create_rename_read() {
    let dir = tempfile::tempdir().unwrap();
    let deep = deep_dir(dir.path());
    let a = deep.join("a.txt");
    let b = deep.join("b.txt");
    assert!(a.as_os_str().len() > 300);

    std::fs::write(to_long_path(&a), b"long").unwrap();
    std::fs::rename(to_long_path(&a), to_long_path(&b)).unwrap();
    let mut s = String::new();
    std::fs::File::open(to_long_path(&b))
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    assert_eq!(s, "long");
    assert!(std::fs::symlink_metadata(to_long_path(&a)).is_err());

    // Attribute and volume helpers accept the plain long path.
    assert!(!is_hidden_or_system(&b));
    set_attrs(&b, FILE_ATTRIBUTE_HIDDEN);
    assert!(is_hidden_or_system(&b));
    set_attrs(&b, FILE_ATTRIBUTE_NORMAL);
    assert!(!is_cloud_placeholder(&b));
    assert!(!is_locked(&b));
    assert!(same_volume(&b, dir.path()).unwrap());
    assert!(can_trash(&b));
}

#[test]
fn long_path_rename_no_replace() {
    let dir = tempfile::tempdir().unwrap();
    let deep = deep_dir(dir.path());
    let a = deep.join("a.txt");
    let b = deep.join("b.txt");
    let c = deep.join("c.txt");
    assert!(a.as_os_str().len() > 300);
    std::fs::write(to_long_path(&a), b"from a").unwrap();
    std::fs::write(to_long_path(&b), b"from b").unwrap();

    // Plain (unprefixed) long paths are accepted.
    let err = rename_no_replace(&a, &b).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(to_long_path(&a)).unwrap(), b"from a");
    assert_eq!(std::fs::read(to_long_path(&b)).unwrap(), b"from b");

    rename_no_replace(&a, &c).unwrap();
    assert!(std::fs::symlink_metadata(to_long_path(&a)).is_err());
    assert_eq!(std::fs::read(to_long_path(&c)).unwrap(), b"from a");
}

#[test]
fn temp_volume_supports_trash() {
    let dir = tempfile::tempdir().unwrap();
    assert!(can_trash(dir.path()));
}

#[test]
fn trash_long_path_never_loses_the_file_silently() {
    let dir = tempfile::tempdir().unwrap();
    let deep = deep_dir(dir.path());
    let f = deep.join("trash-me.txt");
    std::fs::write(to_long_path(&f), b"x").unwrap();
    // The shell may refuse paths over 260 characters. Either outcome is
    // acceptable as long as Ok means "gone from here" and Err means "still here".
    match trash_path(&f) {
        Ok(()) => assert!(std::fs::symlink_metadata(to_long_path(&f)).is_err()),
        Err(e) => {
            eprintln!("trash_path on a long path returned an error: {e}");
            assert!(std::fs::symlink_metadata(to_long_path(&f)).is_ok());
        }
    }
}

#[test]
fn restore_matches_location_ignoring_case_and_separators() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("Restore Case");
    std::fs::create_dir(&sub).unwrap();
    let f = sub.join("Doc.txt");
    std::fs::write(&f, b"case").unwrap();
    let before = std::time::SystemTime::now();
    trash_path(&f).unwrap();

    let other_form = PathBuf::from(
        f.to_string_lossy()
            .replace("Restore Case", "restore case")
            .replace('\\', "/"),
    );
    restore_from_trash(&other_form, before).unwrap();
    assert_eq!(std::fs::read(&f).unwrap(), b"case");
}

#[test]
fn restore_long_path_never_loses_the_file_silently() {
    let dir = tempfile::tempdir().unwrap();
    let deep = deep_dir(dir.path());
    let f = deep.join("restore-me.txt");
    std::fs::write(to_long_path(&f), b"x").unwrap();
    let before = std::time::SystemTime::now();
    if let Err(e) = trash_path(&f) {
        eprintln!("trash_path on a long path returned an error: {e}");
        return;
    }
    // The shell may refuse to restore paths over 260 characters. Ok means
    // "back here"; Err means "not here" (still in the bin).
    match restore_from_trash(&f, before) {
        Ok(()) => assert_eq!(std::fs::read(to_long_path(&f)).unwrap(), b"x"),
        Err(e) => {
            eprintln!("restore_from_trash on a long path returned an error: {e}");
            assert!(std::fs::symlink_metadata(to_long_path(&f)).is_err());
        }
    }
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

fn wide_str(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Writes a value of type `kind` under `HKCU\<subkey>`.
fn reg_set(subkey: &str, name: &str, kind: u32, data: &[u8]) {
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, RegCloseKey, RegCreateKeyExW,
        RegSetValueExW,
    };
    let (subkey, name) = (wide_str(subkey), wide_str(name));
    let mut key = std::ptr::null_mut();
    // SAFETY: NUL-terminated UTF-16 strings, out pointer to a local handle.
    let code = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    assert_eq!(code, 0, "RegCreateKeyExW");
    // SAFETY: `key` is open; `data` has `data.len()` bytes.
    let code = unsafe {
        RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            kind,
            data.as_ptr(),
            data.len() as u32,
        )
    };
    // SAFETY: closes the handle opened above once.
    unsafe { RegCloseKey(key) };
    assert_eq!(code, 0, "RegSetValueExW");
}

/// REG_SZ value under `HKCU\<subkey>`, `None` if missing.
fn reg_get_sz(subkey: &str, name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    let (subkey, name) = (wide_str(subkey), wide_str(name));
    let mut buf = vec![0u16; 2048];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: `buf` has `size` writable bytes.
    let code = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if code == windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND {
        return None;
    }
    assert_eq!(code, 0, "RegGetValueW");
    buf.truncate(size as usize / 2);
    Some(
        String::from_utf16_lossy(&buf)
            .trim_end_matches('\0')
            .to_string(),
    )
}

/// Removes the test's Run, StartupApproved and stale values on drop, also
/// when an assertion fails, so CI never keeps an auto-start entry.
struct AutostartCleanup {
    name: String,
    stale: String,
    exe: PathBuf,
}

impl Drop for AutostartCleanup {
    fn drop(&mut self) {
        let _ = set_autostart_entry(&self.name, &[self.stale.as_str()], false, &self.exe);
    }
}

#[test]
fn autostart_set_read_disable_unset() {
    use windows_sys::Win32::System::Registry::{REG_BINARY, REG_SZ};

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let name = format!("ValetTest-{}-{nanos}", std::process::id());
    let stale = format!("{name}-old");
    let exe = PathBuf::from(r"C:\Program Files Test\Valet Test\Valet.exe");
    let other_exe = PathBuf::from(r"C:\Other\Valet.exe");
    let _cleanup = AutostartCleanup {
        name: name.clone(),
        stale: stale.clone(),
        exe: exe.clone(),
    };

    assert!(!autostart_entry_state(&name, &exe).unwrap());

    // A value left by the old implementation is removed when enabling.
    reg_set(RUN_KEY, &stale, REG_SZ, &[b'x', 0, 0, 0]);
    set_autostart_entry(&name, &[stale.as_str()], true, &exe).unwrap();
    assert_eq!(reg_get_sz(RUN_KEY, &stale), None);
    assert_eq!(
        reg_get_sz(RUN_KEY, &name).as_deref(),
        Some(r#""C:\Program Files Test\Valet Test\Valet.exe" --minimized"#)
    );
    assert!(autostart_entry_state(&name, &exe).unwrap());
    // An entry pointing at another exe is not ours.
    assert!(!autostart_entry_state(&name, &other_exe).unwrap());

    // Task Manager "Disabled": first byte odd. "Enabled": even.
    let mut approved = [0u8; 12];
    approved[0] = 3;
    reg_set(APPROVED_KEY, &name, REG_BINARY, &approved);
    assert!(!autostart_entry_state(&name, &exe).unwrap());
    approved[0] = 2;
    reg_set(APPROVED_KEY, &name, REG_BINARY, &approved);
    assert!(autostart_entry_state(&name, &exe).unwrap());

    // Enabling again clears a Task Manager "Disabled" mark.
    approved[0] = 3;
    reg_set(APPROVED_KEY, &name, REG_BINARY, &approved);
    set_autostart_entry(&name, &[stale.as_str()], true, &exe).unwrap();
    assert!(autostart_entry_state(&name, &exe).unwrap());

    // Disabling removes the value; disabling twice is fine.
    reg_set(RUN_KEY, &stale, REG_SZ, &[b'x', 0, 0, 0]);
    set_autostart_entry(&name, &[stale.as_str()], false, &exe).unwrap();
    set_autostart_entry(&name, &[stale.as_str()], false, &exe).unwrap();
    assert_eq!(reg_get_sz(RUN_KEY, &name), None);
    assert_eq!(reg_get_sz(RUN_KEY, &stale), None);
    assert!(!autostart_entry_state(&name, &exe).unwrap());
}
