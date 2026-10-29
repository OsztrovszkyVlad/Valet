//! Cross-check of the two signers (`docs/LICENSING.md`): the license service
//! (`services/license/lib`, Node) and the core verifier (`minisign-verify`)
//! were written independently. The fixtures in
//! `tests/fixtures/license/server/` come from the service's own code
//! (`make-server-fixtures.mjs`; the token is the real `/v1/activate` answer of
//! its HTTP handler). The other direction (core fixtures verified by the
//! service) is `services/license/test/core-fixtures.test.mjs`.

use std::path::Path;

use valet_core::license::{
    DAY, PlanInputs, PlanStatus, PlanTier, Reason, read_activation, read_license, resolve_plan,
};

// Keep in sync with make-server-fixtures.mjs.
const ISSUED_AT: i64 = 1_795_000_000;
const UPDATES_UNTIL: i64 = ISSUED_AT + 365 * DAY;
const CHECKED_AT: i64 = 1_795_100_000;
const LICENSE_ID: &str = "3e9a5c1d-7b2f-4c8e-9a6d-1f0e2d3c4b5a";
const MACHINE: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
const EMAIL: &str = "server-fixture@example.com";
const NOW: i64 = CHECKED_AT + DAY;
const BUILD: i64 = ISSUED_AT + 10 * DAY;

fn read(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/license")
        .join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn server_key() -> String {
    read("server/TEST-server-pubkey.txt")
}

fn server_license() -> String {
    read("server/server-license.valet-license")
}

fn server_token() -> String {
    read("server/server-token.valet-token")
}

fn resolve(license: &str, token: &str, key: &str) -> PlanStatus {
    resolve_plan(&PlanInputs {
        license: Some(license),
        activation: Some(token),
        machine_id: Some(MACHINE),
        now: NOW,
        build_date: BUILD,
        trial_started_at: Some(NOW - 30 * DAY),
        public_key: Some(key),
    })
}

fn envelope(text: &str) -> (String, String) {
    let v: serde_json::Value = serde_json::from_str(text).unwrap();
    (
        v["payload"].as_str().unwrap().to_owned(),
        v["signature"].as_str().unwrap().to_owned(),
    )
}

fn rebuild(payload: &str, signature: &str) -> String {
    serde_json::json!({ "payload": payload, "signature": signature }).to_string()
}

/// Every envelope that differs from `text` in one byte of the signed payload.
fn payload_tamperings(text: &str) -> Vec<String> {
    let (payload, signature) = envelope(text);
    let bytes = payload.as_bytes();
    assert!(bytes.is_ascii());
    (0..bytes.len())
        .map(|i| {
            let mut b = bytes.to_vec();
            b[i] ^= 0x01;
            rebuild(&String::from_utf8(b).unwrap(), &signature)
        })
        .collect()
}

/// Every envelope that differs from `text` in one character of the
/// signature line (line 2) or the trusted comment and its signature
/// (lines 3 and 4). The last base64 data character of each line is skipped:
/// some of its bits are padding, so a change there may decode to the same
/// bytes.
fn signature_tamperings(text: &str) -> Vec<String> {
    let (payload, signature) = envelope(text);
    let lines: Vec<&str> = signature.split('\n').collect();
    assert_eq!(lines.len(), 5, "4 lines and a final newline");
    let mut out = Vec::new();
    for line_no in [1usize, 3] {
        let line = lines[line_no];
        let data_len = line.trim_end_matches('=').len();
        for i in 0..data_len - 1 {
            let mut chars: Vec<char> = line.chars().collect();
            chars[i] = if chars[i] == 'A' { 'B' } else { 'A' };
            let mut l = lines.clone();
            let changed: String = chars.into_iter().collect();
            l[line_no] = &changed;
            out.push(rebuild(&payload, &l.join("\n")));
        }
    }
    // Trusted comment text (covered by the global signature).
    let trusted = lines[2];
    let prefix = "trusted comment: ".len();
    for i in prefix..trusted.len() {
        let mut b = trusted.as_bytes().to_vec();
        b[i] ^= 0x01;
        let changed = String::from_utf8(b).unwrap();
        let mut l = lines.clone();
        l[2] = &changed;
        out.push(rebuild(&payload, &l.join("\n")));
    }
    out
}

#[test]
fn server_issued_license_and_token_give_pro() {
    let key = server_key();
    let lic = read_license(&server_license(), &key).expect("server license verifies in core");
    assert_eq!(lic.license_id, LICENSE_ID);
    assert_eq!(lic.product, "valet-pro");
    assert_eq!(lic.issued_at, ISSUED_AT);
    assert_eq!(lic.updates_until, UPDATES_UNTIL);
    assert_eq!(lic.max_activations, 3);
    assert_eq!(lic.email, EMAIL);
    let tok = read_activation(&server_token(), &key).expect("server token verifies in core");
    assert_eq!(tok.license_id, LICENSE_ID);
    assert_eq!(tok.machine, MACHINE);
    assert_eq!(tok.checked_at, CHECKED_AT);

    let s = resolve(&server_license(), &server_token(), &key);
    assert_eq!(
        s.plan,
        PlanTier::Pro {
            license_id: LICENSE_ID.into(),
            email: EMAIL.into(),
            updates_until: UPDATES_UNTIL,
        }
    );
    assert!(s.reasons.is_empty(), "{:?}", s.reasons);
    assert!(!s.needs_online_check);
}

#[test]
fn one_byte_payload_tampering_is_rejected() {
    let key = server_key();
    let license = server_license();
    let token = server_token();
    let lic_variants = payload_tamperings(&license);
    let tok_variants = payload_tamperings(&token);
    assert!(lic_variants.len() > 150 && tok_variants.len() > 100);
    for (i, t) in lic_variants.iter().enumerate() {
        assert_eq!(
            read_license(t, &key),
            Err(Reason::LicenseInvalidSignature),
            "byte {i}"
        );
        let s = resolve(t, &token, &key);
        assert_eq!(s.plan, PlanTier::Free, "byte {i}");
        assert_eq!(s.reasons, vec![Reason::LicenseInvalidSignature], "byte {i}");
    }
    for (i, t) in tok_variants.iter().enumerate() {
        assert_eq!(
            read_activation(t, &key),
            Err(Reason::ActivationMissing),
            "byte {i}"
        );
        let s = resolve(&license, t, &key);
        assert_eq!(s.plan, PlanTier::Free, "byte {i}");
        assert_eq!(s.reasons, vec![Reason::ActivationMissing], "byte {i}");
    }
}

#[test]
fn signature_and_trusted_comment_tampering_is_rejected() {
    let key = server_key();
    let license = server_license();
    let token = server_token();
    let lic_variants = signature_tamperings(&license);
    let tok_variants = signature_tamperings(&token);
    assert!(lic_variants.len() > 150);
    for (i, t) in lic_variants.iter().enumerate() {
        assert_eq!(
            read_license(t, &key),
            Err(Reason::LicenseInvalidSignature),
            "variant {i}"
        );
        assert_eq!(resolve(t, &token, &key).plan, PlanTier::Free, "variant {i}");
    }
    for (i, t) in tok_variants.iter().enumerate() {
        assert_eq!(
            read_activation(t, &key),
            Err(Reason::ActivationMissing),
            "variant {i}"
        );
        assert_eq!(
            resolve(&license, t, &key).plan,
            PlanTier::Free,
            "variant {i}"
        );
    }
}

#[test]
fn untrusted_comment_is_not_signed() {
    // Same as minisign: the first line is free text. Changing it keeps the
    // envelope valid, so nothing may be read from it.
    let key = server_key();
    let (payload, signature) = envelope(&server_license());
    let changed = signature.replacen("signature from valet license service", "anything at all", 1);
    assert_ne!(changed, signature);
    assert!(read_license(&rebuild(&payload, &changed), &key).is_ok());
}

#[test]
fn keys_do_not_cross() {
    // Server fixtures with the core fixtures' TEST key, and the other way.
    let core_key = read("TEST-pubkey.txt");
    assert_eq!(
        read_license(&server_license(), &core_key),
        Err(Reason::LicenseInvalidSignature)
    );
    assert_eq!(
        read_activation(&server_token(), &core_key),
        Err(Reason::ActivationMissing)
    );
    let s = resolve(&server_license(), &server_token(), &core_key);
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, vec![Reason::LicenseInvalidSignature]);
    assert_eq!(
        read_license(&read("license-valid.valet-license"), &server_key()),
        Err(Reason::LicenseInvalidSignature)
    );
}

#[test]
fn server_token_for_this_machine_only_and_ages_like_any_token() {
    let key = server_key();
    let at = |now: i64, machine: &str| {
        resolve_plan(&PlanInputs {
            license: Some(&server_license()),
            activation: Some(&server_token()),
            machine_id: Some(machine),
            now,
            build_date: BUILD,
            trial_started_at: Some(NOW - 30 * DAY),
            public_key: Some(&key),
        })
    };
    let other = at(NOW, "fedcba9876543210fedcba9876543210");
    assert_eq!(other.plan, PlanTier::Free);
    assert_eq!(other.reasons, vec![Reason::ActivationOtherMachine]);
    let d31 = at(CHECKED_AT + 31 * DAY, MACHINE);
    assert!(matches!(d31.plan, PlanTier::Pro { .. }), "{d31:?}");
    assert!(d31.needs_online_check);
    let d61 = at(CHECKED_AT + 61 * DAY, MACHINE);
    assert_eq!(d61.plan, PlanTier::Free);
    assert_eq!(d61.reasons, vec![Reason::ActivationExpired]);
}
