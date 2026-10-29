//! License and activation token verification, and plan resolution
//! (`docs/LICENSING.md`). Everything here is pure: the caller passes the file
//! contents, the machine id, the clock and the public key. No file access, no
//! network.

use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};

pub const DAY: i64 = 86_400;
/// Trial length: 14 days from `trial_started_at`.
pub const TRIAL_SECS: i64 = 14 * DAY;
/// A token older than this (`now - checked_at`) no longer gives Pro.
pub const TOKEN_MAX_AGE: i64 = 60 * DAY;
/// From this token age on, the app should ask the server for a new token.
pub const ONLINE_CHECK_AFTER: i64 = 30 * DAY;
/// A token with `checked_at` further than this in the future is invalid.
pub const MAX_FUTURE_SKEW: i64 = DAY;
/// The only product a license may name.
pub const PRODUCT: &str = "valet-pro";
/// Envelopes larger than this are refused before parsing.
const MAX_ENVELOPE_BYTES: usize = 64 * 1024;

fn non_empty(text: &'static str) -> Option<&'static str> {
    let t = text.trim();
    (!t.is_empty()).then_some(t)
}

/// The shipped minisign public key (`license/pubkey.txt`, trimmed). `None`
/// when the file is empty: licensing is not configured.
pub fn shipped_public_key() -> Option<&'static str> {
    non_empty(include_str!("../license/pubkey.txt"))
}

/// The license service base URL (`license/api.txt`, trimmed). `None` when
/// the file is empty: no network calls.
pub fn license_api_base() -> Option<&'static str> {
    non_empty(include_str!("../license/api.txt"))
}

/// The machine id sent to the license service: the first 32 hex characters
/// of `blake3("valet-machine-v1:" + machine_guid)`. The platform layer reads
/// the GUID; only this hash leaves the machine.
pub fn machine_id_from_guid(machine_guid: &str) -> String {
    let hash = blake3::hash(format!("valet-machine-v1:{machine_guid}").as_bytes());
    hash.to_hex()[..32].to_string()
}

/// A build dated `build_date` (Unix seconds; `VALET_BUILD_DATE`, or an
/// update's `pub_date`) is covered by a license with `updates_until`.
pub fn covers(updates_until: i64, build_date: i64) -> bool {
    build_date <= updates_until
}

/// Why the plan is not Pro. Serialized as the contract's code string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Reason {
    #[serde(rename = "license.missing")]
    LicenseMissing,
    #[serde(rename = "license.invalid_signature")]
    LicenseInvalidSignature,
    #[serde(rename = "license.wrong_product")]
    LicenseWrongProduct,
    #[serde(rename = "license.not_covering_build")]
    LicenseNotCoveringBuild,
    #[serde(rename = "activation.missing")]
    ActivationMissing,
    #[serde(rename = "activation.other_machine")]
    ActivationOtherMachine,
    #[serde(rename = "activation.expired")]
    ActivationExpired,
    #[serde(rename = "license.not_configured")]
    LicenseNotConfigured,
}

impl Reason {
    pub fn code(self) -> &'static str {
        match self {
            Reason::LicenseMissing => "license.missing",
            Reason::LicenseInvalidSignature => "license.invalid_signature",
            Reason::LicenseWrongProduct => "license.wrong_product",
            Reason::LicenseNotCoveringBuild => "license.not_covering_build",
            Reason::ActivationMissing => "activation.missing",
            Reason::ActivationOtherMachine => "activation.other_machine",
            Reason::ActivationExpired => "activation.expired",
            Reason::LicenseNotConfigured => "license.not_configured",
        }
    }
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

/// `kind = license` payload, `v = 1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicensePayload {
    pub v: u32,
    pub kind: String,
    pub license_id: String,
    pub product: String,
    pub issued_at: i64,
    pub updates_until: i64,
    pub max_activations: u32,
    pub email: String,
}

/// `kind = activation` payload, `v = 1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationPayload {
    pub v: u32,
    pub kind: String,
    pub license_id: String,
    pub machine: String,
    pub checked_at: i64,
}

#[derive(Deserialize)]
struct Envelope {
    payload: String,
    signature: String,
}

#[derive(Deserialize)]
struct Header {
    v: u64,
    kind: String,
}

/// A minisign public key: the base64 key line, alone or as the last
/// non-empty line of a `minisign.pub` file. `None` when it does not parse.
pub fn parse_public_key(text: &str) -> Option<PublicKey> {
    let line = text.lines().map(str::trim).rfind(|l| !l.is_empty())?;
    PublicKey::from_base64(line).ok()
}

/// The payload text of `envelope` when its prehashed minisign signature over
/// the payload's UTF-8 bytes verifies with `key`. Legacy (`Ed`) signatures
/// are refused.
fn verified_payload(envelope: &str, key: &PublicKey) -> Option<String> {
    if envelope.len() > MAX_ENVELOPE_BYTES {
        return None;
    }
    let env: Envelope = serde_json::from_str(envelope).ok()?;
    let sig = Signature::decode(&env.signature).ok()?;
    key.verify(env.payload.as_bytes(), &sig, false).ok()?;
    Some(env.payload)
}

/// A verified payload with `v = 1` and the given `kind`.
fn verified<T: for<'de> Deserialize<'de>>(
    envelope: &str,
    key: &PublicKey,
    kind: &str,
) -> Option<T> {
    let payload = verified_payload(envelope, key)?;
    let header: Header = serde_json::from_str(&payload).ok()?;
    if header.v != 1 || header.kind != kind {
        return None;
    }
    serde_json::from_str(&payload).ok()
}

/// The license in `envelope` if its signature verifies with `public_key`
/// and it is a `v = 1`, `kind = license` payload. Product and build coverage
/// are checked by [`resolve_plan`].
pub fn read_license(envelope: &str, public_key: &str) -> Result<LicensePayload, Reason> {
    let key = parse_public_key(public_key).ok_or(Reason::LicenseNotConfigured)?;
    verified(envelope, &key, "license").ok_or(Reason::LicenseInvalidSignature)
}

/// The activation token in `envelope` if its signature verifies with
/// `public_key` and it is a `v = 1`, `kind = activation` payload.
pub fn read_activation(envelope: &str, public_key: &str) -> Result<ActivationPayload, Reason> {
    let key = parse_public_key(public_key).ok_or(Reason::LicenseNotConfigured)?;
    verified(envelope, &key, "activation").ok_or(Reason::ActivationMissing)
}

/// The plan in effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlanTier {
    Free,
    Trial {
        ends_at: i64,
    },
    Pro {
        license_id: String,
        email: String,
        updates_until: i64,
    },
}

impl PlanTier {
    /// Free limits apply (`docs/LICENSING.md`, Plans).
    pub fn is_free(&self) -> bool {
        matches!(self, PlanTier::Free)
    }
}

/// The license shown in Settings, whenever its signature is valid (also when
/// it gives no Pro: `updates_until` is the date for
/// `license.not_covering_build`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub license_id: String,
    pub email: String,
    pub updates_until: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStatus {
    pub plan: PlanTier,
    /// Every reason Pro does not apply, in check order. Empty for Pro.
    pub reasons: Vec<Reason>,
    /// The license is valid for this build and the token belongs to it and
    /// this machine, but the token is at least 30 days old or dated more
    /// than a day in the future: the app should call `/v1/validate`.
    pub needs_online_check: bool,
    pub license: Option<LicenseInfo>,
    /// `trial_started_at + 14 days`, also after the trial ended.
    pub trial_ends_at: Option<i64>,
}

/// Inputs of [`resolve_plan`]. Times are Unix seconds (UTC).
#[derive(Debug, Clone, Copy)]
pub struct PlanInputs<'a> {
    /// Contents of `license.valet-license`, if the file exists.
    pub license: Option<&'a str>,
    /// Contents of `activation.valet-token`, if the file exists.
    pub activation: Option<&'a str>,
    /// This machine's id ([`machine_id_from_guid`]), if it could be read.
    pub machine_id: Option<&'a str>,
    pub now: i64,
    /// `VALET_BUILD_DATE` of the running build.
    pub build_date: i64,
    /// `meta.trial_started_at`.
    pub trial_started_at: Option<i64>,
    /// [`shipped_public_key`] in the app; a TEST key in tests. `None` or a
    /// key that does not parse: licensing is not configured.
    pub public_key: Option<&'a str>,
}

/// Plan resolution from `docs/LICENSING.md`: Pro when the license and the
/// token are valid, else Trial while it lasts, else Free.
pub fn resolve_plan(inputs: &PlanInputs<'_>) -> PlanStatus {
    let mut reasons = Vec::new();
    let mut license = None;
    let mut pro = None;
    let mut needs_online_check = false;

    match inputs.public_key.and_then(parse_public_key) {
        None => reasons.push(Reason::LicenseNotConfigured),
        Some(key) => match inputs.license {
            None => reasons.push(Reason::LicenseMissing),
            Some(text) => match verified::<LicensePayload>(text, &key, "license") {
                None => reasons.push(Reason::LicenseInvalidSignature),
                Some(lic) => {
                    let info = LicenseInfo {
                        license_id: lic.license_id.clone(),
                        email: lic.email.clone(),
                        updates_until: lic.updates_until,
                    };
                    let mut license_ok = true;
                    if lic.product != PRODUCT {
                        reasons.push(Reason::LicenseWrongProduct);
                        license_ok = false;
                    }
                    if !covers(lic.updates_until, inputs.build_date) {
                        reasons.push(Reason::LicenseNotCoveringBuild);
                        license_ok = false;
                    }
                    let token = inputs
                        .activation
                        .and_then(|t| verified::<ActivationPayload>(t, &key, "activation"));
                    match token {
                        // A token for another license is no activation of this one.
                        None => reasons.push(Reason::ActivationMissing),
                        Some(t) if t.license_id != lic.license_id => {
                            reasons.push(Reason::ActivationMissing)
                        }
                        Some(t) if inputs.machine_id != Some(t.machine.as_str()) => {
                            reasons.push(Reason::ActivationOtherMachine)
                        }
                        Some(t) => {
                            let age = inputs.now.saturating_sub(t.checked_at);
                            let future = age < -MAX_FUTURE_SKEW;
                            needs_online_check =
                                license_ok && (future || age >= ONLINE_CHECK_AFTER);
                            if future || age > TOKEN_MAX_AGE {
                                reasons.push(Reason::ActivationExpired);
                            } else if license_ok {
                                pro = Some(PlanTier::Pro {
                                    license_id: info.license_id.clone(),
                                    email: info.email.clone(),
                                    updates_until: info.updates_until,
                                });
                            }
                        }
                    }
                    license = Some(info);
                }
            },
        },
    }

    let trial_ends_at = inputs
        .trial_started_at
        .map(|s| s.saturating_add(TRIAL_SECS));
    let plan = match (pro, trial_ends_at) {
        (Some(p), _) => p,
        (None, Some(end)) if inputs.now < end => PlanTier::Trial { ends_at: end },
        (None, _) => PlanTier::Free,
    };
    PlanStatus {
        plan,
        reasons,
        needs_online_check,
        license,
        trial_ends_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_files_are_empty_so_licensing_is_not_configured() {
        assert_eq!(shipped_public_key(), None);
        assert_eq!(license_api_base(), None);
    }

    #[test]
    fn machine_id_is_32_lowercase_hex_and_depends_on_the_guid() {
        let a = machine_id_from_guid("4c4c4544-0042-3510-8052-b4c04f4e4d32");
        assert_eq!(a.len(), 32);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_eq!(
            a,
            machine_id_from_guid("4c4c4544-0042-3510-8052-b4c04f4e4d32")
        );
        assert_ne!(
            a,
            machine_id_from_guid("4c4c4544-0042-3510-8052-b4c04f4e4d33")
        );
        let full = blake3::hash(b"valet-machine-v1:x").to_hex();
        assert_eq!(machine_id_from_guid("x"), &full[..32]);
    }

    #[test]
    fn covers_includes_the_last_day() {
        assert!(covers(100, 99));
        assert!(covers(100, 100));
        assert!(!covers(100, 101));
    }

    #[test]
    fn reason_serializes_as_its_code() {
        for r in [
            Reason::LicenseMissing,
            Reason::LicenseInvalidSignature,
            Reason::LicenseWrongProduct,
            Reason::LicenseNotCoveringBuild,
            Reason::ActivationMissing,
            Reason::ActivationOtherMachine,
            Reason::ActivationExpired,
            Reason::LicenseNotConfigured,
        ] {
            assert_eq!(serde_json::to_value(r).unwrap(), r.code());
            assert_eq!(r.to_string(), r.code());
        }
    }

    #[test]
    fn plan_tier_serializes_with_a_kind_tag() {
        assert_eq!(
            serde_json::to_value(PlanTier::Trial { ends_at: 5 }).unwrap(),
            serde_json::json!({ "kind": "trial", "ends_at": 5 })
        );
        assert_eq!(
            serde_json::to_value(PlanTier::Free).unwrap(),
            serde_json::json!({ "kind": "free" })
        );
    }

    #[test]
    fn public_key_parses_from_bare_line_or_minisign_pub_file() {
        assert!(parse_public_key("").is_none());
        assert!(parse_public_key("not base64").is_none());
        let line = "RWQiAvAxCK84QnJf8+Igdwm9G7dN3O4Kos4iF+q02XktRin27S2NaAVe";
        assert!(parse_public_key(line).is_some());
        assert!(parse_public_key(&format!("{line}\n")).is_some());
        assert!(
            parse_public_key(&format!("untrusted comment: minisign public key\n{line}\n"))
                .is_some()
        );
    }
}
