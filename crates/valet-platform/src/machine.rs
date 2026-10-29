//! Machine id for license activation (`docs/LICENSING.md`, "Activation
//! token"): a hash of the OS machine GUID, so the raw GUID never leaves the
//! machine.

/// Prefix hashed before the machine GUID. Changing it changes every
/// machine id and invalidates every activation.
const MACHINE_ID_PREFIX: &str = "valet-machine-v1:";

/// Hex characters kept from the blake3 hash.
const MACHINE_ID_LEN: usize = 32;

/// The machine id: the first 32 lowercase hex characters of
/// `blake3("valet-machine-v1:" + guid)`, where `guid` is the machine GUID
/// trimmed and lowercased.
///
/// Windows: `MachineGuid` (REG_SZ) under `HKLM\SOFTWARE\Microsoft\Cryptography`,
/// read only, from the 64-bit registry view. Off Windows (Linux development
/// and e2e only): the content of `/etc/machine-id`. `None` when the value is
/// missing, unreadable or empty.
pub fn machine_id() -> Option<String> {
    read_machine_guid().and_then(|guid| machine_id_from_guid(&guid))
}

/// [`machine_id`] for a given GUID string. `None` if it is empty after
/// trimming.
pub fn machine_id_from_guid(guid: &str) -> Option<String> {
    let guid = guid.trim().to_lowercase();
    if guid.is_empty() {
        return None;
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(MACHINE_ID_PREFIX.as_bytes());
    hasher.update(guid.as_bytes());
    let mut hex = hasher.finalize().to_hex().to_string();
    hex.truncate(MACHINE_ID_LEN);
    Some(hex)
}

#[cfg(windows)]
fn read_machine_guid() -> Option<String> {
    use std::ptr;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_WOW64_64KEY, RRF_RT_REG_SZ, RegCloseKey,
        RegGetValueW, RegOpenKeyExW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let subkey = wide(r"SOFTWARE\Microsoft\Cryptography");
    let name = wide("MachineGuid");
    let mut key: HKEY = ptr::null_mut();
    // SAFETY: `subkey` is NUL-terminated UTF-16 and `key` receives the
    // handle. Query-only access, 64-bit view.
    let code = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            subkey.as_ptr(),
            0,
            KEY_QUERY_VALUE | KEY_WOW64_64KEY,
            &mut key,
        )
    };
    if code != ERROR_SUCCESS {
        return None;
    }
    // A GUID is 36 characters; a longer value fails with ERROR_MORE_DATA
    // and gives `None`.
    let mut buf = [0u16; 128];
    let mut size = std::mem::size_of_val(&buf) as u32;
    // SAFETY: `key` is open with KEY_QUERY_VALUE, `name` is NUL-terminated,
    // `buf` has `size` writable bytes. RRF_RT_REG_SZ accepts only REG_SZ and
    // NUL-terminates the data.
    let code = unsafe {
        RegGetValueW(
            key,
            ptr::null(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    // SAFETY: `key` was opened above and is closed once.
    unsafe { RegCloseKey(key) };
    if code != ERROR_SUCCESS {
        return None;
    }
    let chars = (size as usize / 2).min(buf.len());
    let data = &buf[..chars];
    let end = data.iter().position(|&c| c == 0).unwrap_or(data.len());
    String::from_utf16(&data[..end]).ok()
}

#[cfg(not(windows))]
fn read_machine_guid() -> Option<String> {
    std::fs::read_to_string("/etc/machine-id").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_id(s: &str) -> bool {
        s.len() == MACHINE_ID_LEN && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    }

    #[test]
    fn id_is_the_prefixed_blake3_hash_truncated() {
        let guid = "6f1c2a3b-4d5e-4f60-8a9b-0c1d2e3f4a5b";
        let full = blake3::hash(format!("valet-machine-v1:{guid}").as_bytes())
            .to_hex()
            .to_string();
        let id = machine_id_from_guid(guid).unwrap();
        assert_eq!(id, full[..32]);
        assert!(is_id(&id));
    }

    #[test]
    fn id_is_fixed_for_a_known_guid() {
        // Pinned so a change to the prefix, case or trimming is noticed:
        // it would move every activation to a new machine.
        assert_eq!(
            machine_id_from_guid("6f1c2a3b-4d5e-4f60-8a9b-0c1d2e3f4a5b").unwrap(),
            "861af0ec91cc426015ce3289e5baece3"
        );
    }

    #[test]
    fn guid_is_trimmed_and_lowercased() {
        let plain = machine_id_from_guid("6f1c2a3b-4d5e-4f60-8a9b-0c1d2e3f4a5b").unwrap();
        assert_eq!(
            machine_id_from_guid("  6F1C2A3B-4D5E-4F60-8A9B-0C1D2E3F4A5B\r\n").unwrap(),
            plain
        );
        assert_ne!(
            machine_id_from_guid("6f1c2a3b-4d5e-4f60-8a9b-0c1d2e3f4a5c").unwrap(),
            plain
        );
    }

    #[test]
    fn empty_guid_gives_none() {
        assert_eq!(machine_id_from_guid(""), None);
        assert_eq!(machine_id_from_guid(" \n\t"), None);
    }

    #[test]
    fn machine_id_is_stable_and_well_formed() {
        let first = machine_id();
        if let Some(id) = &first {
            assert!(is_id(id), "{id}");
        }
        assert_eq!(machine_id(), first);
    }

    #[cfg(windows)]
    #[test]
    fn machine_id_reads_machine_guid_on_windows() {
        let id = machine_id().expect("MachineGuid must be readable on Windows");
        assert!(is_id(&id), "{id}");
    }
}
