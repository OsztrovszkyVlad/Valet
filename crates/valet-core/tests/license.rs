//! Licensing contract (`docs/LICENSING.md`): signature checks, plan
//! resolution, plan limits, rule export and import.
//!
//! Signed fixtures live in `tests/fixtures/license/` and come from
//! `make-fixtures.mjs` (run it with Node to regenerate them byte for byte).
//! They are signed with TEST keys; no TEST public key is ever shipped.
//! Every test passes `now` explicitly.

use std::path::Path;

use proptest::prelude::*;
use serde_json::json;
use uuid::Uuid;
use valet_core::{
    entitlements::{
        LimitKind, Limits, PLAN_DISABLED, PlanLimit, apply_free_limits, batch_visible, can_undo,
        check_folders, check_rule_change, restore_after_upgrade, split_active_folders,
        visible_batch_cutoff,
    },
    license::{
        DAY, PlanInputs, PlanStatus, PlanTier, Reason, TRIAL_SECS, read_activation, read_license,
        resolve_plan,
    },
    rule_transfer::{ImportError, MAX_IMPORT_BYTES, MAX_IMPORT_RULES, export_rules, import_rules},
    rules::Rule,
};

// Keep in sync with make-fixtures.mjs.
const NOW: i64 = 1_794_000_000;
const BUILD: i64 = 1_793_000_000;
const LICENSE_ID: &str = "6f1c2a3b-4d5e-4f60-8a71-92b3c4d5e6f7";
const MACHINE: &str = "0123456789abcdef0123456789abcdef";
const EMAIL: &str = "test@example.com";
const UPDATES_UNTIL: i64 = 1_821_536_000;
const OLD_UPDATES_UNTIL: i64 = 1_781_536_000;
/// Trial started long ago: ended.
const TRIAL_ENDED: Option<i64> = Some(NOW - 30 * DAY);

fn fixture(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/license")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn test_key() -> String {
    fixture("TEST-pubkey.txt")
}

/// Resolution with the TEST key, this machine, `NOW`, `BUILD`, trial ended.
fn resolve(license: Option<&str>, token: Option<&str>) -> PlanStatus {
    resolve_with(license, token, |_| {})
}

fn resolve_with(
    license: Option<&str>,
    token: Option<&str>,
    tweak: impl FnOnce(&mut PlanInputs<'_>),
) -> PlanStatus {
    let license = license.map(fixture);
    let token = token.map(fixture);
    let key = test_key();
    let mut inputs = PlanInputs {
        license: license.as_deref(),
        activation: token.as_deref(),
        machine_id: Some(MACHINE),
        now: NOW,
        build_date: BUILD,
        trial_started_at: TRIAL_ENDED,
        public_key: Some(&key),
    };
    tweak(&mut inputs);
    resolve_plan(&inputs)
}

fn pro() -> PlanTier {
    PlanTier::Pro {
        license_id: LICENSE_ID.into(),
        email: EMAIL.into(),
        updates_until: UPDATES_UNTIL,
    }
}

const VALID: &str = "license-valid.valet-license";
const TOKEN: &str = "token-valid.valet-token";

// ---------------------------------------------------------------- signatures

#[test]
fn valid_license_and_token_read_with_the_test_key() {
    let lic = read_license(&fixture(VALID), &test_key()).unwrap();
    assert_eq!(lic.license_id, LICENSE_ID);
    assert_eq!(lic.product, "valet-pro");
    assert_eq!(lic.updates_until, UPDATES_UNTIL);
    assert_eq!(lic.max_activations, 3);
    assert_eq!(lic.email, EMAIL);
    let tok = read_activation(&fixture(TOKEN), &test_key()).unwrap();
    assert_eq!(tok.license_id, LICENSE_ID);
    assert_eq!(tok.machine, MACHINE);
    assert_eq!(tok.checked_at, NOW - DAY);
}

#[test]
fn bad_license_files_are_refused_as_invalid_signature() {
    for name in [
        "license-tampered.valet-license",
        "license-other-key.valet-license",
        "license-forged-same-key-id.valet-license",
        "license-legacy.valet-license",
        "license-malformed.valet-license",
        "license-v2.valet-license",
        TOKEN, // kind = activation
    ] {
        assert_eq!(
            read_license(&fixture(name), &test_key()),
            Err(Reason::LicenseInvalidSignature),
            "{name}"
        );
    }
}

#[test]
fn bad_tokens_are_refused_as_missing_activation() {
    for name in [
        "token-tampered.valet-token",
        "token-other-key.valet-token",
        VALID, // kind = license
    ] {
        assert_eq!(
            read_activation(&fixture(name), &test_key()),
            Err(Reason::ActivationMissing),
            "{name}"
        );
    }
}

#[test]
fn valid_files_fail_with_another_public_key() {
    let other = fixture("TEST-other-pubkey.txt");
    assert_eq!(
        read_license(&fixture(VALID), &other),
        Err(Reason::LicenseInvalidSignature)
    );
    assert!(read_license(&fixture("license-other-key.valet-license"), &other).is_ok());
}

#[test]
fn unusable_public_key_means_not_configured() {
    for key in ["", "   ", "garbage"] {
        assert_eq!(
            read_license(&fixture(VALID), key),
            Err(Reason::LicenseNotConfigured)
        );
    }
}

#[test]
fn signature_covers_the_exact_payload_bytes() {
    // Re-serializing the payload (same JSON value, other bytes) breaks it.
    let mut env: serde_json::Value = serde_json::from_str(&fixture(VALID)).unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(env["payload"].as_str().unwrap()).unwrap();
    env["payload"] = serde_json::to_string_pretty(&payload).unwrap().into();
    assert_eq!(
        read_license(&env.to_string(), &test_key()),
        Err(Reason::LicenseInvalidSignature)
    );
}

#[test]
fn oversized_envelope_is_refused() {
    let mut env: serde_json::Value = serde_json::from_str(&fixture(VALID)).unwrap();
    env["padding"] = "x".repeat(70 * 1024).into();
    assert_eq!(
        read_license(&env.to_string(), &test_key()),
        Err(Reason::LicenseInvalidSignature)
    );
}

// ---------------------------------------------------------------- resolution

#[test]
fn valid_license_and_fresh_token_give_pro() {
    let s = resolve(Some(VALID), Some(TOKEN));
    assert_eq!(s.plan, pro());
    assert!(s.reasons.is_empty());
    assert!(!s.needs_online_check);
    assert_eq!(s.license.unwrap().email, EMAIL);
}

#[test]
fn token_31_days_old_is_still_pro_and_asks_for_a_check() {
    let s = resolve(Some(VALID), Some("token-31-days.valet-token"));
    assert_eq!(s.plan, pro());
    assert!(s.reasons.is_empty());
    assert!(s.needs_online_check);
}

#[test]
fn token_61_days_old_is_free_and_expired() {
    let s = resolve(Some(VALID), Some("token-61-days.valet-token"));
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::ActivationExpired]);
    assert!(s.needs_online_check, "a check can restore Pro");
    assert!(s.license.is_some(), "the license is still shown");
}

#[test]
fn token_age_boundaries_30_and_60_days() {
    let checked_at = NOW - DAY;
    let at = |now: i64| resolve_with(Some(VALID), Some(TOKEN), |i| i.now = now);
    let s = at(checked_at + 30 * DAY - 1);
    assert!(!s.needs_online_check);
    let s = at(checked_at + 30 * DAY);
    assert!(s.needs_online_check);
    assert_eq!(at(checked_at + 60 * DAY).plan, pro());
    let s = at(checked_at + 60 * DAY + 1);
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::ActivationExpired]);
}

#[test]
fn token_two_days_in_the_future_is_invalid() {
    let s = resolve(Some(VALID), Some("token-future-2-days.valet-token"));
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::ActivationExpired]);
    assert!(s.needs_online_check);
}

#[test]
fn token_up_to_one_day_in_the_future_is_accepted() {
    let checked_at = NOW - DAY;
    let s = resolve_with(Some(VALID), Some(TOKEN), |i| i.now = checked_at - DAY);
    assert_eq!(s.plan, pro());
    let s = resolve_with(Some(VALID), Some(TOKEN), |i| i.now = checked_at - DAY - 1);
    assert_eq!(s.reasons, [Reason::ActivationExpired]);
}

#[test]
fn token_for_another_machine_gives_other_machine() {
    let s = resolve(Some(VALID), Some("token-other-machine.valet-token"));
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::ActivationOtherMachine]);
    assert!(!s.needs_online_check);
    // No machine id at all: no token can match.
    let s = resolve_with(Some(VALID), Some(TOKEN), |i| i.machine_id = None);
    assert_eq!(s.reasons, [Reason::ActivationOtherMachine]);
}

#[test]
fn token_for_another_license_counts_as_missing() {
    let s = resolve(Some(VALID), Some("token-other-license.valet-token"));
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::ActivationMissing]);
}

#[test]
fn license_without_token_is_activation_missing() {
    let s = resolve(Some(VALID), None);
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::ActivationMissing]);
    for bad in ["token-tampered.valet-token", "token-other-key.valet-token"] {
        assert_eq!(
            resolve(Some(VALID), Some(bad)).reasons,
            [Reason::ActivationMissing]
        );
    }
}

#[test]
fn wrong_product_is_free() {
    let s = resolve(Some("license-wrong-product.valet-license"), Some(TOKEN));
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::LicenseWrongProduct]);
    assert!(!s.needs_online_check);
}

#[test]
fn license_not_covering_the_build_is_free_and_names_the_date() {
    let s = resolve(
        Some("license-not-covering-build.valet-license"),
        Some(TOKEN),
    );
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::LicenseNotCoveringBuild]);
    assert_eq!(s.license.unwrap().updates_until, OLD_UPDATES_UNTIL);
    // An older build is covered by the same license.
    let s = resolve_with(
        Some("license-not-covering-build.valet-license"),
        Some(TOKEN),
        |i| i.build_date = OLD_UPDATES_UNTIL,
    );
    assert!(matches!(s.plan, PlanTier::Pro { .. }));
}

#[test]
fn build_dated_on_updates_until_is_covered_one_second_later_is_not() {
    let s = resolve_with(Some(VALID), Some(TOKEN), |i| i.build_date = UPDATES_UNTIL);
    assert_eq!(s.plan, pro());
    let s = resolve_with(Some(VALID), Some(TOKEN), |i| {
        i.build_date = UPDATES_UNTIL + 1
    });
    assert_eq!(s.reasons, [Reason::LicenseNotCoveringBuild]);
}

#[test]
fn every_invalid_license_file_gives_invalid_signature() {
    for name in [
        "license-tampered.valet-license",
        "license-other-key.valet-license",
        "license-forged-same-key-id.valet-license",
        "license-legacy.valet-license",
        "license-malformed.valet-license",
        "license-v2.valet-license",
    ] {
        let s = resolve(Some(name), Some(TOKEN));
        assert_eq!(s.plan, PlanTier::Free, "{name}");
        assert_eq!(s.reasons, [Reason::LicenseInvalidSignature], "{name}");
        assert!(s.license.is_none(), "{name}");
        assert!(!s.needs_online_check, "{name}");
    }
}

#[test]
fn no_license_gives_license_missing() {
    let s = resolve(None, Some(TOKEN));
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::LicenseMissing]);
}

#[test]
fn no_public_key_means_not_configured_and_trial_still_works() {
    let s = resolve_with(Some(VALID), Some(TOKEN), |i| {
        i.public_key = None;
        i.trial_started_at = Some(NOW);
    });
    assert_eq!(
        s.plan,
        PlanTier::Trial {
            ends_at: NOW + TRIAL_SECS
        }
    );
    assert_eq!(s.reasons, [Reason::LicenseNotConfigured]);
    let s = resolve_with(Some(VALID), Some(TOKEN), |i| i.public_key = Some(""));
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.reasons, [Reason::LicenseNotConfigured]);
}

#[test]
fn several_license_problems_are_all_reported() {
    // Not covering this build and a token for another machine.
    let s = resolve(
        Some("license-not-covering-build.valet-license"),
        Some("token-other-machine.valet-token"),
    );
    assert_eq!(
        s.reasons,
        [
            Reason::LicenseNotCoveringBuild,
            Reason::ActivationOtherMachine
        ]
    );
}

#[test]
fn trial_lasts_exactly_14_days() {
    let start = NOW - 10 * DAY;
    let at = |now: i64| {
        resolve_with(None, None, |i| {
            i.now = now;
            i.trial_started_at = Some(start);
        })
    };
    let end = start + 14 * 86_400;
    assert_eq!(at(start).plan, PlanTier::Trial { ends_at: end });
    assert_eq!(at(end - 1).plan, PlanTier::Trial { ends_at: end });
    let s = at(end);
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.trial_ends_at, Some(end));
    assert_eq!(s.reasons, [Reason::LicenseMissing]);
    // No trial start known: Free.
    let s = resolve_with(None, None, |i| i.trial_started_at = None);
    assert_eq!(s.plan, PlanTier::Free);
    assert_eq!(s.trial_ends_at, None);
}

#[test]
fn pro_wins_over_a_running_trial_and_trial_reports_why_not_pro() {
    let s = resolve_with(Some(VALID), Some(TOKEN), |i| i.trial_started_at = Some(NOW));
    assert_eq!(s.plan, pro());
    let s = resolve_with(Some(VALID), Some("token-61-days.valet-token"), |i| {
        i.trial_started_at = Some(NOW)
    });
    assert_eq!(
        s.plan,
        PlanTier::Trial {
            ends_at: NOW + TRIAL_SECS
        }
    );
    assert_eq!(s.reasons, [Reason::ActivationExpired]);
}

#[test]
fn plan_status_serializes_with_snake_case_and_codes() {
    let s = resolve(Some(VALID), Some("token-61-days.valet-token"));
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["plan"], json!({ "kind": "free" }));
    assert_eq!(v["reasons"], json!(["activation.expired"]));
    assert_eq!(v["needs_online_check"], json!(true));
    assert_eq!(v["license"]["updates_until"], json!(UPDATES_UNTIL));
    let v = serde_json::to_value(resolve(Some(VALID), Some(TOKEN)).plan).unwrap();
    assert_eq!(
        v,
        json!({ "kind": "pro", "license_id": LICENSE_ID, "email": EMAIL, "updates_until": UPDATES_UNTIL })
    );
}

// ---------------------------------------------------------------- limits

fn rule(n: u128, enabled: bool, preset: Option<&str>) -> Rule {
    let options = match preset {
        Some(p) => json!({ "presetId": p }),
        None => json!({}),
    };
    serde_json::from_value(json!({
        "id": Uuid::from_u128(n),
        "name": format!("r{n}"),
        "enabled": enabled,
        "conditions": [{ "type": "ext", "value": "pdf" }],
        "actions": [{ "type": "report" }],
        "options": options,
        "version": 7,
    }))
    .unwrap()
}

const FREE: PlanTier = PlanTier::Free;
const TRIAL: PlanTier = PlanTier::Trial { ends_at: NOW };

fn rules_limit() -> Result<(), PlanLimit> {
    Err(PlanLimit {
        kind: LimitKind::Rules,
        limit: 3,
    })
}

#[test]
fn limits_table_matches_the_contract() {
    let f = Limits::for_plan(&FREE);
    assert_eq!(
        (
            f.own_rules,
            f.folders,
            f.history_days,
            f.undo_latest_only,
            f.export_import
        ),
        (Some(3), Some(2), Some(7), true, false)
    );
    for p in [TRIAL, pro()] {
        let l = Limits::for_plan(&p);
        assert_eq!(
            (
                l.own_rules,
                l.folders,
                l.history_days,
                l.undo_latest_only,
                l.export_import
            ),
            (None, None, None, false, true)
        );
    }
}

#[test]
fn free_allows_a_third_own_rule_and_refuses_a_fourth() {
    let two = vec![rule(1, true, None), rule(2, true, None)];
    assert_eq!(check_rule_change(&two, &rule(3, true, None), &FREE), Ok(()));
    let three = vec![
        rule(1, true, None),
        rule(2, true, None),
        rule(3, true, None),
    ];
    assert_eq!(
        check_rule_change(&three, &rule(4, true, None), &FREE),
        rules_limit()
    );
    // Enabling a disabled own rule is refused the same way.
    let mut with_off = three.clone();
    with_off.push(rule(4, false, None));
    assert_eq!(
        check_rule_change(&with_off, &rule(4, true, None), &FREE),
        rules_limit()
    );
    // Trial and Pro: no limit.
    assert_eq!(
        check_rule_change(&with_off, &rule(4, true, None), &TRIAL),
        Ok(())
    );
    assert_eq!(
        check_rule_change(&with_off, &rule(9, true, None), &pro()),
        Ok(())
    );
}

#[test]
fn free_always_allows_disabling_editing_and_presets() {
    let mut rules = vec![
        rule(1, true, None),
        rule(2, true, None),
        rule(3, true, None),
    ];
    rules.push(rule(4, false, None));
    // Adding a disabled rule, editing a disabled rule, disabling one.
    assert_eq!(
        check_rule_change(&rules, &rule(5, false, None), &FREE),
        Ok(())
    );
    assert_eq!(
        check_rule_change(&rules, &rule(4, false, None), &FREE),
        Ok(())
    );
    assert_eq!(
        check_rule_change(&rules, &rule(1, false, None), &FREE),
        Ok(())
    );
    // Editing an enabled rule that stays enabled.
    let mut renamed = rule(2, true, None);
    renamed.name = "renamed".into();
    assert_eq!(check_rule_change(&rules, &renamed, &FREE), Ok(()));
    // Presets are not counted, in the list or as the candidate.
    for n in 10..18 {
        rules.push(rule(n, true, Some("downloads-by-type")));
    }
    assert_eq!(
        check_rule_change(&rules, &rule(20, true, Some("invoices")), &FREE),
        Ok(())
    );
    let two_own_many_presets: Vec<Rule> = rules
        .iter()
        .filter(|r| r.id != Uuid::from_u128(3))
        .cloned()
        .collect();
    assert_eq!(
        check_rule_change(&two_own_many_presets, &rule(4, true, None), &FREE),
        Ok(())
    );
    // A preset rule turned into an own rule counts as enabling one.
    let mut own = rule(10, true, None);
    own.name = "mine now".into();
    assert_eq!(check_rule_change(&rules, &own, &FREE), rules_limit());
}

fn enabled_ids(rules: &[Rule]) -> Vec<u128> {
    rules
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.id.as_u128())
        .collect()
}

fn apply(rules: &mut [Rule], changed: Vec<Rule>) {
    for c in changed {
        let slot = rules.iter_mut().find(|r| r.id == c.id).unwrap();
        *slot = c;
    }
}

#[test]
fn downgrade_keeps_first_three_own_rules_and_upgrade_restores_the_rest() {
    let mut rules = vec![
        rule(1, true, None),
        rule(2, false, None), // disabled by the user
        rule(3, true, None),
        rule(4, true, None),
        rule(5, true, None),
        rule(6, true, None),
        rule(10, true, Some("invoices")),
        rule(11, true, Some("screenshots")),
    ];
    let before = enabled_ids(&rules);
    let changed = apply_free_limits(&rules);
    assert_eq!(
        changed.iter().map(|r| r.id.as_u128()).collect::<Vec<_>>(),
        [5, 6]
    );
    assert!(
        changed
            .iter()
            .all(|r| !r.enabled && r.option_flag(PLAN_DISABLED))
    );
    assert!(changed.iter().all(|r| r.version == 7), "version untouched");
    apply(&mut rules, changed);
    assert_eq!(enabled_ids(&rules), [1, 3, 4, 10, 11]);
    // Idempotent.
    assert!(apply_free_limits(&rules).is_empty());

    let changed = restore_after_upgrade(&rules);
    assert_eq!(
        changed.iter().map(|r| r.id.as_u128()).collect::<Vec<_>>(),
        [5, 6]
    );
    assert!(
        changed
            .iter()
            .all(|r| r.options.get(PLAN_DISABLED).is_none())
    );
    apply(&mut rules, changed);
    assert_eq!(enabled_ids(&rules), before);
    assert!(!rules[1].enabled, "user-disabled rule stays disabled");
    assert!(restore_after_upgrade(&rules).is_empty());
}

#[test]
fn downgrade_with_three_or_fewer_own_rules_changes_nothing() {
    let rules = vec![
        rule(1, true, None),
        rule(2, true, None),
        rule(3, true, None),
        rule(4, false, None),
        rule(10, true, Some("invoices")),
    ];
    assert!(apply_free_limits(&rules).is_empty());
    assert!(restore_after_upgrade(&rules).is_empty());
}

#[test]
fn stale_plan_disabled_key_is_cleaned_without_enabling_twice() {
    // Enabled rule still carrying the flag (the user re-enabled it).
    let mut r = rule(1, true, None);
    r.options = json!({ "planDisabled": true, "topLevelOnly": true });
    let changed = apply_free_limits(std::slice::from_ref(&r));
    assert_eq!(changed.len(), 1);
    assert!(changed[0].enabled);
    assert_eq!(changed[0].options, json!({ "topLevelOnly": true }));
    let changed = restore_after_upgrade(&[r]);
    assert!(changed[0].enabled);
    assert_eq!(changed[0].options, json!({ "topLevelOnly": true }));
    // `planDisabled: false` is removed without enabling.
    let mut off = rule(2, false, None);
    off.options = json!({ "planDisabled": false });
    let changed = restore_after_upgrade(&[off]);
    assert!(!changed[0].enabled);
    assert_eq!(changed[0].options, json!({}));
}

#[test]
fn free_refuses_adding_a_third_folder_but_allows_removing_and_reordering() {
    let p = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let one = p(&["C:\\A"]);
    let two = p(&["C:\\A", "C:\\B"]);
    let three = p(&["C:\\A", "C:\\B", "C:\\C"]);
    let folders_limit = Err(PlanLimit {
        kind: LimitKind::Folders,
        limit: 2,
    });
    assert_eq!(check_folders(&one, &two, &FREE), Ok(()));
    assert_eq!(check_folders(&two, &three, &FREE), folders_limit);
    assert_eq!(check_folders(&two, &three, &TRIAL), Ok(()));
    // After a downgrade the config may hold more than 2.
    let four = p(&["C:\\A", "C:\\B", "C:\\C", "C:\\D"]);
    assert_eq!(check_folders(&four, &three, &FREE), Ok(()), "remove");
    let reordered = p(&["C:\\C", "C:\\D", "C:\\A", "C:\\B"]);
    assert_eq!(check_folders(&four, &reordered, &FREE), Ok(()), "reorder");
    let swapped = p(&["C:\\A", "C:\\B", "C:\\C", "C:\\E"]);
    assert_eq!(
        check_folders(&four, &swapped, &FREE),
        folders_limit,
        "swap in new"
    );
}

#[test]
fn free_keeps_first_two_folders_active() {
    let all: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
    let (active, inactive) = split_active_folders(&all, &FREE);
    assert_eq!(active, ["a", "b"]);
    assert_eq!(inactive, ["c", "d"]);
    let (active, inactive) = split_active_folders(&all[..2], &FREE);
    assert_eq!((active.len(), inactive.len()), (2, 0));
    let (active, inactive) = split_active_folders(&all, &pro());
    assert_eq!((active, inactive.len()), (all.clone(), 0));
}

#[test]
fn history_cutoff_is_seven_days_on_free_only() {
    assert_eq!(visible_batch_cutoff(&FREE, NOW), Some(NOW - 7 * 86_400));
    assert_eq!(visible_batch_cutoff(&TRIAL, NOW), None);
    assert_eq!(visible_batch_cutoff(&pro(), NOW), None);
    // 2026-10-29T00:00:00Z = 1793232000.
    let now = 1_793_232_000 + 7 * DAY;
    let cut = visible_batch_cutoff(&FREE, now);
    assert!(batch_visible("2026-10-29T00:00:00.000Z", cut));
    assert!(!batch_visible("2026-10-28T23:59:59.999Z", cut));
    assert!(batch_visible("2020-01-01T00:00:00.000Z", None));
}

#[test]
fn free_undoes_only_the_latest_batch() {
    assert!(can_undo("b2", Some("b2"), &FREE));
    assert!(!can_undo("b1", Some("b2"), &FREE));
    assert!(!can_undo("b1", None, &FREE));
    assert!(can_undo("b1", Some("b2"), &TRIAL));
    assert!(can_undo("b1", Some("b2"), &pro()));
}

proptest! {
    /// Downgrade then upgrade, with no user change in between, enables
    /// exactly the rules that were enabled before.
    #[test]
    fn downgrade_then_upgrade_restores_the_enabled_set(
        spec in proptest::collection::vec((any::<bool>(), any::<bool>()), 0..20)
    ) {
        let mut rules: Vec<Rule> = spec
            .iter()
            .enumerate()
            .map(|(i, &(enabled, preset))| {
                rule(i as u128 + 1, enabled, preset.then_some("invoices"))
            })
            .collect();
        // Evaluation order: own rules first.
        rules.sort_by_key(Rule::is_preset);
        let before = enabled_ids(&rules);
        let changed = apply_free_limits(&rules);
        apply(&mut rules, changed);
        let own_on = rules.iter().filter(|r| r.enabled && !r.is_preset()).count();
        prop_assert!(own_on <= 3);
        prop_assert!(apply_free_limits(&rules).is_empty());
        let changed = restore_after_upgrade(&rules);
        apply(&mut rules, changed);
        prop_assert_eq!(enabled_ids(&rules), before);
        prop_assert!(restore_after_upgrade(&rules).is_empty());
        prop_assert!(rules.iter().all(|r| r.options.get(PLAN_DISABLED).is_none()));
    }
}

// ---------------------------------------------------------------- export / import

fn accept(_: &Rule) -> Result<(), &'static str> {
    Ok(())
}

#[test]
fn export_import_round_trip_keeps_content_with_new_ids() {
    let mut a = rule(1, true, None);
    a.scope = vec!["{downloads}".into()];
    a.options = json!({ "topLevelOnly": true });
    let b = rule(2, false, None);
    let preset = rule(3, true, Some("invoices"));
    let text = export_rules(&[a.clone(), preset, b.clone()]);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["format"], "valet-rules");
    assert_eq!(v["v"], 1);
    assert_eq!(v["rules"].as_array().unwrap().len(), 2, "presets left out");

    let imported = import_rules(&text, accept).unwrap();
    assert_eq!(imported.len(), 2);
    for (got, orig) in imported.iter().zip([&a, &b]) {
        assert_ne!(got.id, orig.id, "new id");
        assert_eq!(got.version, 1);
        let mut same = got.clone();
        same.id = orig.id;
        assert!(same.same_content(orig), "{got:?}");
    }
    assert_ne!(imported[0].id, imported[1].id);
}

#[test]
fn importing_the_same_file_twice_never_reuses_an_id() {
    let text = export_rules(&[rule(1, true, None)]);
    let first = import_rules(&text, accept).unwrap();
    let second = import_rules(&text, accept).unwrap();
    assert_ne!(first[0].id, second[0].id);
    assert_ne!(first[0].id, Uuid::from_u128(1));
}

#[test]
fn import_makes_rules_own_and_undoes_plan_disabling() {
    let mut r = rule(1, false, None);
    r.options = json!({ "presetId": "invoices", "planDisabled": true, "includeHidden": true });
    let text = json!({ "format": "valet-rules", "v": 1, "rules": [r] }).to_string();
    let got = import_rules(&text, accept).unwrap();
    assert!(!got[0].is_preset());
    assert!(got[0].enabled);
    assert_eq!(got[0].options, json!({ "includeHidden": true }));
}

#[test]
fn import_rejects_bad_files() {
    let ok_rule = serde_json::to_value(rule(1, true, None)).unwrap();
    let cases: Vec<(String, ImportError)> = vec![
        ("not json".into(), ImportError::Malformed),
        ("[]".into(), ImportError::UnknownFormat),
        (
            json!({ "format": "other", "v": 1, "rules": [] }).to_string(),
            ImportError::UnknownFormat,
        ),
        (
            json!({ "format": "valet-rules", "v": 2, "rules": [] }).to_string(),
            ImportError::UnknownFormat,
        ),
        (
            json!({ "format": "valet-rules", "rules": [] }).to_string(),
            ImportError::UnknownFormat,
        ),
        (
            json!({ "format": "valet-rules", "v": 1 }).to_string(),
            ImportError::Malformed,
        ),
        (
            json!({ "format": "valet-rules", "v": 1, "rules": {} }).to_string(),
            ImportError::Malformed,
        ),
        (
            json!({ "format": "valet-rules", "v": 1, "rules": [ok_rule, { "name": "x" }] })
                .to_string(),
            ImportError::Malformed,
        ),
    ];
    for (text, want) in cases {
        assert_eq!(
            import_rules(&text, accept).err(),
            Some(want.clone()),
            "{text}"
        );
    }
    let empty = json!({ "format": "valet-rules", "v": 1, "rules": [] }).to_string();
    assert_eq!(import_rules(&empty, accept).map(|v| v.len()), Ok(0));
}

#[test]
fn import_enforces_size_and_count_caps() {
    let big = format!(
        "{{\"format\":\"valet-rules\",\"v\":1,\"rules\":[],\"pad\":\"{}\"}}",
        "x".repeat(MAX_IMPORT_BYTES)
    );
    assert_eq!(
        import_rules(&big, accept).err(),
        Some(ImportError::TooLarge)
    );
    let many: Vec<Rule> = (0..=MAX_IMPORT_RULES as u128)
        .map(|n| rule(n, false, None))
        .collect();
    let text = json!({ "format": "valet-rules", "v": 1, "rules": many }).to_string();
    assert!(text.len() <= MAX_IMPORT_BYTES, "count cap is what fails");
    assert_eq!(
        import_rules(&text, accept).err(),
        Some(ImportError::TooManyRules)
    );
    let max: Vec<Rule> = many[..MAX_IMPORT_RULES].to_vec();
    let text = json!({ "format": "valet-rules", "v": 1, "rules": max }).to_string();
    assert_eq!(import_rules(&text, accept).unwrap().len(), MAX_IMPORT_RULES);
}

#[test]
fn import_runs_the_validator_on_every_rule_as_stored() {
    let mut bad = rule(2, true, None);
    bad.name = "  ".into();
    let mut preset = rule(3, true, Some("invoices"));
    preset.name = "p".into();
    let text =
        json!({ "format": "valet-rules", "v": 1, "rules": [rule(1, true, None), bad] }).to_string();
    let validate = |r: &Rule| {
        if r.name.trim().is_empty() {
            Err("rule.nameEmpty")
        } else {
            Ok(())
        }
    };
    let err = import_rules(&text, validate).unwrap_err();
    assert_eq!(
        err,
        ImportError::InvalidRule {
            index: 1,
            code: "rule.nameEmpty"
        }
    );
    assert_eq!(err.code(), "import.invalidRule");
    assert_eq!(err.to_string(), "import.invalidRule: 2 rule.nameEmpty");
    // The validator sees the rule after normalization (no presetId, new id).
    let text = json!({ "format": "valet-rules", "v": 1, "rules": [preset] }).to_string();
    let seen = std::cell::RefCell::new(Vec::new());
    import_rules(&text, |r: &Rule| {
        seen.borrow_mut().push((r.is_preset(), r.id, r.version));
        Ok(())
    })
    .unwrap();
    let seen = seen.into_inner();
    assert!(!seen[0].0);
    assert_ne!(seen[0].1, Uuid::from_u128(3));
    assert_eq!(seen[0].2, 1);
}
