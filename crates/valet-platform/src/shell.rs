//! Shell integration: showing a file in Explorer.

use std::io;
use std::path::Path;

/// Opens an Explorer window on the folder of `path` with `path` selected.
///
/// Errors with `NotFound` if `path` does not exist (checked through
/// [`crate::to_long_path`], so paths over 260 characters work) and with
/// `InvalidInput` if it is relative or not valid Unicode. Explorer itself
/// does not accept `\\?\` paths, so it gets the path without that prefix.
///
/// Runs `explorer.exe /select,"<path>"` directly, without a shell. The
/// quotes are written as a raw argument because Explorer does not follow the
/// usual argument quoting; a Windows path cannot contain `"`, so the path
/// cannot end the quoted part early. Explorer's exit code is not meaningful
/// (it is 1 on success), so the call returns once the process started.
///
/// Off Windows: `Unsupported` after the existence check.
pub fn reveal_in_explorer(path: &Path) -> io::Result<()> {
    std::fs::symlink_metadata(crate::to_long_path(path))?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let s = path.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "path is not valid Unicode")
        })?;
        std::process::Command::new("explorer.exe")
            .raw_arg(explorer_select_arg(s)?)
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(windows))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "showing a file in Explorer is only supported on Windows",
        ))
    }
}

/// The `/select,"<path>"` argument for `explorer.exe`, with any `\\?\` or
/// `\\?\UNC\` prefix removed and `/` turned into `\`.
#[cfg_attr(not(windows), allow(dead_code))]
fn explorer_select_arg(path: &str) -> io::Result<String> {
    if path.contains('"') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains a quote",
        ));
    }
    let path = path.replace('/', "\\");
    let plain = if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path
    };
    let is_drive_absolute = plain.as_bytes().get(1) == Some(&b':')
        && plain.as_bytes().get(2) == Some(&b'\\')
        && plain.as_bytes()[0].is_ascii_alphabetic();
    if !(is_drive_absolute || plain.starts_with(r"\\")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is not absolute",
        ));
    }
    Ok(format!("/select,\"{plain}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_arg_quotes_the_path() {
        assert_eq!(
            explorer_select_arg(r"C:\Users\Ana Pop\Documente\a, b.pdf").unwrap(),
            r#"/select,"C:\Users\Ana Pop\Documente\a, b.pdf""#
        );
    }

    #[test]
    fn select_arg_strips_long_path_prefix() {
        assert_eq!(
            explorer_select_arg(r"\\?\C:\x\y.txt").unwrap(),
            r#"/select,"C:\x\y.txt""#
        );
        assert_eq!(
            explorer_select_arg(r"\\?\UNC\srv\share\y.txt").unwrap(),
            r#"/select,"\\srv\share\y.txt""#
        );
        assert_eq!(
            explorer_select_arg("C:/x/y.txt").unwrap(),
            r#"/select,"C:\x\y.txt""#
        );
    }

    #[test]
    fn select_arg_keeps_long_paths_whole() {
        let long = format!(r"C:\{}\f.txt", "d".repeat(300));
        assert_eq!(
            explorer_select_arg(&format!(r"\\?\{long}")).unwrap(),
            format!("/select,\"{long}\"")
        );
    }

    #[test]
    fn select_arg_rejects_quotes_and_relative_paths() {
        for bad in [
            r#"C:\a" /e,C:\b"#,
            r"relative\f.txt",
            r"\rooted\f.txt",
            "C:f.txt",
        ] {
            let err = explorer_select_arg(bad).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{bad}");
        }
    }

    #[test]
    fn missing_path_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let err = reveal_in_explorer(&dir.path().join("missing.txt")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[cfg(not(windows))]
    #[test]
    fn existing_path_is_unsupported_off_windows() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f.txt");
        std::fs::write(&f, b"x").unwrap();
        let err = reveal_in_explorer(&f).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Unsupported);
    }
}
