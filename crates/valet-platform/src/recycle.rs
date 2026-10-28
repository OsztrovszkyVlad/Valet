//! Sending files to the Recycle Bin (Windows) or the freedesktop trash (Linux).

use std::io;
use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
