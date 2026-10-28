//! Per-user auto-start through `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
//!
//! Only `HKEY_CURRENT_USER` is read or written; Valet installs per user and
//! never needs admin rights. The entry is the value [`AUTOSTART_VALUE_NAME`]
//! with the data `"<exe>" --minimized` (exe path in quotes, so paths with
//! spaces start the right program).
//!
//! Task Manager's Startup tab does not remove the Run value when the user
//! disables an entry; it writes a binary value of the same name under
//! `...\Explorer\StartupApproved\Run` whose first byte is odd. That counts as
//! disabled here.

use std::io;
use std::path::Path;

/// Value name of the auto-start entry under the Run key.
pub const AUTOSTART_VALUE_NAME: &str = "Valet";

/// Argument the auto-start entry passes to Valet: start hidden in the tray.
pub const AUTOSTART_ARG: &str = "--minimized";

/// Value name an earlier build wrote under the Run key. Removed whenever
/// auto-start is turned on or off.
const STALE_VALUE_NAMES: &[&str] = &["ValetFileOrganizer"];

#[cfg_attr(not(windows), allow(dead_code))]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg_attr(not(windows), allow(dead_code))]
const APPROVED_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// Turns the per-user auto-start entry for `exe` on or off.
///
/// On: writes `"<exe>" --minimized` as the REG_SZ value `Valet` under
/// `HKCU\...\Run` and removes a `Valet` value under `StartupApproved\Run`, so
/// an entry the user disabled earlier in Task Manager starts again. Off:
/// removes both values. Both directions also remove the stale
/// `ValetFileOrganizer` Run value. Removing a value that does not exist is
/// not an error.
///
/// Errors with `InvalidInput` if `exe` is relative, not valid Unicode or
/// contains `"` (checked before any registry write).
/// Off Windows: `Unsupported`.
pub fn set_autostart(enabled: bool, exe: &Path) -> io::Result<()> {
    set_autostart_entry(AUTOSTART_VALUE_NAME, STALE_VALUE_NAMES, enabled, exe)
}

/// True if the auto-start entry exists, its data equals the command
/// [`set_autostart`] writes for `exe` (ASCII case ignored), and it is not
/// disabled under `StartupApproved\Run` (first byte odd; a missing or empty
/// value counts as enabled).
///
/// Off Windows: `Unsupported`.
pub fn autostart_state(exe: &Path) -> io::Result<bool> {
    autostart_entry_state(AUTOSTART_VALUE_NAME, exe)
}

/// [`set_autostart`] with a chosen value name and stale names. Tests use a
/// unique name so they never touch the real entry.
#[doc(hidden)]
pub fn set_autostart_entry(
    name: &str,
    stale_names: &[&str],
    enabled: bool,
    exe: &Path,
) -> io::Result<()> {
    let command = run_command(exe)?;
    #[cfg(windows)]
    {
        for stale in stale_names {
            reg::delete_value(RUN_KEY, stale)?;
        }
        if enabled {
            reg::set_string(RUN_KEY, name, &command)?;
        } else {
            reg::delete_value(RUN_KEY, name)?;
        }
        reg::delete_value(APPROVED_KEY, name)
    }
    #[cfg(not(windows))]
    {
        let _ = (name, stale_names, enabled, command);
        Err(unsupported())
    }
}

/// [`autostart_state`] with a chosen value name.
#[doc(hidden)]
pub fn autostart_entry_state(name: &str, exe: &Path) -> io::Result<bool> {
    let command = run_command(exe)?;
    #[cfg(windows)]
    {
        let Some(current) = reg::get_string(RUN_KEY, name)? else {
            return Ok(false);
        };
        if !current.trim().eq_ignore_ascii_case(&command) {
            return Ok(false);
        }
        Ok(!approved_disabled(
            reg::get_binary(APPROVED_KEY, name)?.as_deref(),
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = (name, command);
        Err(unsupported())
    }
}

#[cfg(not(windows))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "auto-start is only supported on Windows",
    )
}

/// True if a `StartupApproved\Run` value marks the entry disabled: first byte
/// odd. Missing or empty data means enabled.
#[cfg_attr(not(windows), allow(dead_code))]
fn approved_disabled(data: Option<&[u8]>) -> bool {
    data.and_then(|d| d.first()).is_some_and(|b| b & 1 == 1)
}

/// The Run value data for `exe`: `"<exe>" --minimized`. A `\\?\` or
/// `\\?\UNC\` prefix is removed and `/` becomes `\`.
fn run_command(exe: &Path) -> io::Result<String> {
    let s = exe
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path is not valid Unicode"))?;
    if s.contains('"') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains a quote",
        ));
    }
    let s = s.replace('/', "\\");
    let plain = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s
    };
    let b = plain.as_bytes();
    let drive_absolute = b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
    if !(drive_absolute || plain.starts_with(r"\\")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path is not absolute",
        ));
    }
    Ok(format!("\"{plain}\" {AUTOSTART_ARG}"))
}

#[cfg(windows)]
mod reg {
    //! Minimal HKCU value access. Every call opens the key, does one thing
    //! and closes it.

    use std::io;
    use std::ptr;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_BINARY,
        RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegGetValueW,
        RegSetValueExW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn check(code: u32) -> io::Result<()> {
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(code as i32))
        }
    }

    /// Writes `data` as REG_SZ value `name` under `HKCU\<subkey>`, creating
    /// the key if needed.
    pub(super) fn set_string(subkey: &str, name: &str, data: &str) -> io::Result<()> {
        let subkey = wide(subkey);
        let name = wide(name);
        let data = wide(data);
        let mut key: HKEY = ptr::null_mut();
        // SAFETY: `subkey` is NUL-terminated UTF-16, `key` receives the
        // handle, null class/security/disposition are allowed.
        check(unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                ptr::null(),
                &mut key,
                ptr::null_mut(),
            )
        })?;
        // SAFETY: `key` is open with KEY_SET_VALUE; `data` is NUL-terminated
        // UTF-16 and the byte count includes the terminator, as REG_SZ needs.
        let result = check(unsafe {
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        });
        // SAFETY: `key` was opened above and is closed once.
        unsafe { RegCloseKey(key) };
        result
    }

    /// Removes value `name` under `HKCU\<subkey>`. A missing key or value
    /// is not an error.
    pub(super) fn delete_value(subkey: &str, name: &str) -> io::Result<()> {
        let subkey = wide(subkey);
        let name = wide(name);
        // SAFETY: both arguments are NUL-terminated UTF-16.
        let code = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, subkey.as_ptr(), name.as_ptr()) };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(code)
    }

    /// Raw bytes of value `name` under `HKCU\<subkey>` restricted to
    /// `flags` types; `None` if the key or value is missing.
    fn get_raw(subkey: &str, name: &str, flags: u32) -> io::Result<Option<Vec<u8>>> {
        let subkey = wide(subkey);
        let name = wide(name);
        loop {
            let mut size: u32 = 0;
            // SAFETY: NUL-terminated strings; a null data pointer asks for
            // the size only.
            let code = unsafe {
                RegGetValueW(
                    HKEY_CURRENT_USER,
                    subkey.as_ptr(),
                    name.as_ptr(),
                    flags,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut size,
                )
            };
            if code == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            check(code)?;
            let mut buf = vec![0u8; size as usize];
            let mut got = size;
            // SAFETY: `buf` has `got` writable bytes.
            let code = unsafe {
                RegGetValueW(
                    HKEY_CURRENT_USER,
                    subkey.as_ptr(),
                    name.as_ptr(),
                    flags,
                    ptr::null_mut(),
                    buf.as_mut_ptr().cast(),
                    &mut got,
                )
            };
            match code {
                ERROR_FILE_NOT_FOUND => return Ok(None),
                // The value grew between the two calls; ask again.
                windows_sys::Win32::Foundation::ERROR_MORE_DATA => continue,
                _ => check(code)?,
            }
            buf.truncate(got as usize);
            return Ok(Some(buf));
        }
    }

    /// REG_SZ value `name` under `HKCU\<subkey>`; `None` if missing. A value
    /// of another type is an error (`ERROR_UNSUPPORTED_TYPE`).
    pub(super) fn get_string(subkey: &str, name: &str) -> io::Result<Option<String>> {
        let Some(bytes) = get_raw(subkey, name, RRF_RT_REG_SZ)? else {
            return Ok(None);
        };
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .take_while(|&u| u != 0)
            .collect();
        Ok(Some(String::from_utf16_lossy(&units)))
    }

    /// REG_BINARY value `name` under `HKCU\<subkey>`; `None` if missing or
    /// of another type.
    pub(super) fn get_binary(subkey: &str, name: &str) -> io::Result<Option<Vec<u8>>> {
        match get_raw(subkey, name, RRF_RT_REG_BINARY) {
            Err(e)
                if e.raw_os_error()
                    == Some(windows_sys::Win32::Foundation::ERROR_UNSUPPORTED_TYPE as i32) =>
            {
                Ok(None)
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_quotes_the_exe_and_adds_minimized() {
        assert_eq!(
            run_command(Path::new(r"C:\Users\Ana Pop\AppData\Local\Valet\Valet.exe")).unwrap(),
            r#""C:\Users\Ana Pop\AppData\Local\Valet\Valet.exe" --minimized"#
        );
    }

    #[test]
    fn command_strips_long_path_prefix() {
        assert_eq!(
            run_command(Path::new(r"\\?\C:\Valet\Valet.exe")).unwrap(),
            r#""C:\Valet\Valet.exe" --minimized"#
        );
        assert_eq!(
            run_command(Path::new(r"\\?\UNC\srv\share\Valet.exe")).unwrap(),
            r#""\\srv\share\Valet.exe" --minimized"#
        );
    }

    #[test]
    fn command_rejects_quotes_and_relative_paths() {
        for bad in [
            r#"C:\a"b\Valet.exe"#,
            r"Valet.exe",
            r"\Valet.exe",
            "C:Valet.exe",
        ] {
            let err = run_command(Path::new(bad)).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{bad}");
        }
    }

    #[test]
    fn approved_first_byte_parity() {
        assert!(!approved_disabled(None));
        assert!(!approved_disabled(Some(&[])));
        assert!(!approved_disabled(Some(&[2, 0, 0, 0])));
        assert!(!approved_disabled(Some(&[6, 0, 0, 0])));
        assert!(approved_disabled(Some(&[3, 0, 0, 0])));
        assert!(approved_disabled(Some(&[1])));
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_off_windows() {
        let exe = Path::new(r"C:\Valet\Valet.exe");
        assert_eq!(
            set_autostart(true, exe).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            autostart_state(exe).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }
}
