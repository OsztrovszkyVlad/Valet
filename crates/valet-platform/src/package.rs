//! Whether Valet runs with a package identity (the Microsoft Store MSIX
//! build). That build hides the updater and the HKCU auto-start entry.

/// True if the process has a package identity (MSIX): `GetCurrentPackageFullName`
/// succeeds or asks for a larger buffer. `APPMODEL_ERROR_NO_PACKAGE` and any
/// other error give false.
///
/// Off Windows: false.
pub fn is_packaged() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
        use windows_sys::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;

        let mut len: u32 = 0;
        // SAFETY: a zero length with a null buffer asks for the length only.
        let code = unsafe { GetCurrentPackageFullName(&mut len, std::ptr::null_mut()) };
        // APPMODEL_ERROR_NO_PACKAGE (15700) when unpackaged.
        matches!(code, ERROR_SUCCESS | ERROR_INSUFFICIENT_BUFFER)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tests_run_unpackaged() {
        // `cargo test` binaries never have a package identity.
        assert!(!is_packaged());
    }
}
