//! Windows-only checks against the real file system (run on windows-latest CI).
#![cfg(windows)]

use std::ffi::OsStr;
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use valet_platform::{
    can_trash, is_cloud_placeholder, is_hidden_or_system, is_locked, is_reparse_point, same_volume,
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
