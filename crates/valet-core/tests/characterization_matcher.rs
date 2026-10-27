//! Characterization tests for `valet_core::matcher`.
//!
//! These pin the behavior of `rule_matches` / `matching_rules` as it is
//! today (Phase 0), including behavior that looks wrong. Assertions marked
//! `// characterization: current behavior, ...` describe a suspected bug or
//! gap; change them deliberately when the matcher is rewritten.

mod common;

use std::path::{Path, PathBuf};

use common::{cond_rule, rule};
use serde_json::json;
use valet_core::matcher::{FileFacts, matching_rules, rule_matches};
use valet_core::rules::Rule;

fn p(parts: &[&str]) -> PathBuf {
    let mut out = PathBuf::from("root");
    for part in parts {
        out = out.join(part);
    }
    out
}

fn matches_path(rule: &Rule, path: &Path, size: u64) -> bool {
    let s = path.to_string_lossy();
    rule_matches(rule, &FileFacts { path: &s, size })
}

fn matches_str(rule: &Rule, path: &str, size: u64) -> bool {
    rule_matches(rule, &FileFacts { path, size })
}

// ---------------------------------------------------------------- enabled / alwaysApply

#[test]
fn char_disabled_rule_never_matches() {
    let mut r = cond_rule("ext", json!("pdf"));
    r.enabled = false;
    assert!(!matches_path(&r, &p(&["a.pdf"]), 10));

    // Disabled wins over alwaysApply.
    r.always_apply = true;
    assert!(!matches_path(&r, &p(&["a.pdf"]), 10));
}

#[test]
fn char_always_apply_ignores_conditions() {
    let mut r = cond_rule("ext", json!("pdf"));
    r.always_apply = true;
    assert!(matches_path(&r, &p(&["a.txt"]), 10));
    assert!(matches_path(&r, &p(&["no_extension"]), 0));
}

#[test]
fn char_always_apply_deserializes_from_camel_case() {
    let r: Rule = serde_json::from_value(json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "name": "all",
        "enabled": true,
        "alwaysApply": true,
        "conditions": [{ "type": "ext", "value": "never" }],
        "actions": []
    }))
    .unwrap();
    assert!(r.always_apply);
    assert!(matches_path(&r, &p(&["a.txt"]), 1));
}

#[test]
fn char_empty_conditions_match_every_file() {
    let r = rule(1, "empty", json!([]), json!([]));
    // characterization: current behavior, an enabled rule with no conditions matches every file (Iterator::all on empty list); a rule saved without conditions would act on the whole scope
    assert!(matches_path(&r, &p(&["a.pdf"]), 10));
    assert!(matches_path(&r, &p(&["sub", "anything"]), 0));
}

// ---------------------------------------------------------------- ext

#[test]
fn char_ext_match_and_mismatch() {
    let r = cond_rule("ext", json!("pdf"));
    assert!(matches_path(&r, &p(&["a.pdf"]), 1));
    assert!(!matches_path(&r, &p(&["a.txt"]), 1));
    assert!(!matches_path(&r, &p(&["pdf"]), 1));
    assert!(!matches_path(&r, &p(&["a.pdfx"]), 1));
}

#[test]
fn char_ext_case_insensitive_both_ways() {
    let lower = cond_rule("ext", json!("pdf"));
    assert!(matches_path(&lower, &p(&["A.PDF"]), 1));
    assert!(matches_path(&lower, &p(&["a.Pdf"]), 1));

    let upper = cond_rule("ext", json!("PDF"));
    assert!(matches_path(&upper, &p(&["a.pdf"]), 1));
    assert!(matches_path(&upper, &p(&["A.PDF"]), 1));
}

#[test]
fn char_ext_value_with_leading_dot_never_matches() {
    let r = cond_rule("ext", json!(".pdf"));
    // characterization: current behavior, a value with a leading dot (".pdf") does not match "a.pdf"; the dot is not stripped
    assert!(!matches_path(&r, &p(&["a.pdf"]), 1));
}

#[test]
fn char_ext_file_without_extension() {
    let r = cond_rule("ext", json!("pdf"));
    assert!(!matches_path(&r, &p(&["README"]), 1));

    let empty = cond_rule("ext", json!(""));
    assert!(!matches_path(&empty, &p(&["README"]), 1));

    // Dotfile: Path::extension() is None for ".pdf".
    assert!(!matches_path(&r, &p(&[".pdf"]), 1));
}

#[test]
fn char_ext_empty_value_matches_trailing_dot_name() {
    // "a." has extension Some("") per std::path, so an empty value matches it.
    let empty = cond_rule("ext", json!(""));
    // characterization: current behavior, ext "" matches a file named "a." (empty extension)
    assert!(matches_path(&empty, &p(&["a."]), 1));
    assert!(!matches_path(&empty, &p(&["a.pdf"]), 1));
}

#[test]
fn char_ext_double_extension_only_last_part() {
    let gz = cond_rule("ext", json!("gz"));
    assert!(matches_path(&gz, &p(&["a.tar.gz"]), 1));

    let tar_gz = cond_rule("ext", json!("tar.gz"));
    // characterization: current behavior, multi-part extensions ("tar.gz") never match; only the last component is compared
    assert!(!matches_path(&tar_gz, &p(&["a.tar.gz"]), 1));
}

#[test]
fn char_ext_non_string_value() {
    // Non-string value is read as "".
    let num = cond_rule("ext", json!(5));
    assert!(!matches_path(&num, &p(&["a.5"]), 1));
    assert!(!matches_path(&num, &p(&["a.pdf"]), 1));
    assert!(!matches_path(&num, &p(&["README"]), 1));

    let arr = cond_rule("ext", json!(["pdf", "doc"]));
    // characterization: current behavior, an array of extensions is silently treated as "" and never matches a normal file (no error surfaced)
    assert!(!matches_path(&arr, &p(&["a.pdf"]), 1));

    let null = cond_rule("ext", serde_json::Value::Null);
    // characterization: current behavior, non-string/null ext value behaves like "" and matches a trailing-dot name "a."
    assert!(matches_path(&null, &p(&["a."]), 1));
}

#[test]
fn char_ext_missing_value_field_defaults_to_null() {
    let r: Rule = serde_json::from_value(json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "name": "r",
        "enabled": true,
        "conditions": [{ "type": "ext" }],
        "actions": []
    }))
    .unwrap();
    assert!(r.conditions[0].value.is_null());
    assert!(!matches_path(&r, &p(&["a.pdf"]), 1));
}

// ---------------------------------------------------------------- nameMatches

#[test]
fn char_name_matches_file_name_only() {
    let r = cond_rule("nameMatches", json!("invoice"));
    assert!(matches_path(&r, &p(&["docs", "invoice_01.pdf"]), 1));
    // Pattern appears only in the parent directory: no match.
    assert!(!matches_path(&r, &p(&["invoice_dir", "photo.jpg"]), 1));

    // Anchors apply to the file name, not the full path.
    let anchored = cond_rule("nameMatches", json!("^photo\\.jpg$"));
    assert!(matches_path(
        &anchored,
        &p(&["invoice_dir", "photo.jpg"]),
        1
    ));
}

#[test]
fn char_name_matches_is_case_sensitive_unless_flag() {
    let plain = cond_rule("nameMatches", json!("invoice"));
    assert!(!matches_path(&plain, &p(&["INVOICE.pdf"]), 1));

    let flagged = cond_rule("nameMatches", json!("(?i)invoice"));
    assert!(matches_path(&flagged, &p(&["INVOICE.pdf"]), 1));
    assert!(matches_path(&flagged, &p(&["Invoice.pdf"]), 1));
}

#[test]
fn char_name_matches_invalid_regex_is_false() {
    let r = cond_rule("nameMatches", json!("([unclosed"));
    // characterization: current behavior, an invalid regex silently evaluates to false (no error surfaced to the user); regex is also recompiled for every file
    assert!(!matches_path(&r, &p(&["([unclosed"]), 1));
    assert!(!matches_path(&r, &p(&["a.pdf"]), 1));
}

#[test]
fn char_name_matches_non_string_value_is_empty_pattern() {
    let r = cond_rule("nameMatches", json!(42));
    // characterization: current behavior, a non-string pattern becomes "" which matches every file name
    assert!(matches_path(&r, &p(&["anything.txt"]), 1));
}

#[test]
fn char_name_matches_unicode_name() {
    let r = cond_rule("nameMatches", json!("^Factură"));
    assert!(matches_path(&r, &p(&["Factură.pdf"]), 1));
    assert!(!matches_path(&r, &p(&["Factura.pdf"]), 1));

    // `(?i)` in the regex crate is Unicode-aware: Ă matches ă.
    let ci = cond_rule("nameMatches", json!("(?i)^FACTURĂ"));
    assert!(matches_path(&ci, &p(&["Factură.pdf"]), 1));
}

// ---------------------------------------------------------------- sizeGt / sizeLt

#[test]
fn char_size_boundaries_are_strict() {
    let gt = cond_rule("sizeGt", json!(100));
    assert!(!matches_path(&gt, &p(&["a"]), 99));
    assert!(!matches_path(&gt, &p(&["a"]), 100));
    assert!(matches_path(&gt, &p(&["a"]), 101));

    let lt = cond_rule("sizeLt", json!(100));
    assert!(matches_path(&lt, &p(&["a"]), 99));
    assert!(!matches_path(&lt, &p(&["a"]), 100));
    assert!(!matches_path(&lt, &p(&["a"]), 101));
}

#[test]
fn char_size_negative_value_wraps_to_u64() {
    let gt = cond_rule("sizeGt", json!(-1));
    // characterization: current behavior, -1 is cast to u64::MAX, so sizeGt -1 never matches (even a 0-byte file)
    assert!(!matches_path(&gt, &p(&["a"]), 0));
    assert!(!matches_path(&gt, &p(&["a"]), 1_000_000));

    let lt = cond_rule("sizeLt", json!(-1));
    // characterization: current behavior, -1 is cast to u64::MAX, so sizeLt -1 matches every file
    assert!(matches_path(&lt, &p(&["a"]), 0));
    assert!(matches_path(&lt, &p(&["a"]), 1_000_000_000));
}

#[test]
fn char_size_non_numeric_value_is_zero() {
    let gt = cond_rule("sizeGt", json!("100"));
    // characterization: current behavior, a string "100" is read as 0, so sizeGt "100" matches any non-empty file
    assert!(matches_path(&gt, &p(&["a"]), 1));
    assert!(!matches_path(&gt, &p(&["a"]), 0));

    let lt = cond_rule("sizeLt", json!("100"));
    // characterization: current behavior, a string "100" is read as 0, so sizeLt "100" never matches
    assert!(!matches_path(&lt, &p(&["a"]), 0));
    assert!(!matches_path(&lt, &p(&["a"]), 50));
}

#[test]
fn char_size_float_value_is_zero() {
    let gt = cond_rule("sizeGt", json!(100.5));
    // characterization: current behavior, a float value is read as 0 (as_i64 fails), so sizeGt 100.5 matches a 1-byte file
    assert!(matches_path(&gt, &p(&["a"]), 1));

    let gt_whole = cond_rule("sizeGt", json!(100.0));
    // characterization: current behavior, even a whole float (100.0) is read as 0
    assert!(matches_path(&gt_whole, &p(&["a"]), 1));

    let lt = cond_rule("sizeLt", json!(100.5));
    // characterization: current behavior, sizeLt with a float never matches
    assert!(!matches_path(&lt, &p(&["a"]), 1));
}

#[test]
fn char_size_value_above_i64_max_is_zero() {
    let lt = cond_rule("sizeLt", json!(u64::MAX));
    // characterization: current behavior, values above i64::MAX are read as 0, so sizeLt u64::MAX never matches
    assert!(!matches_path(&lt, &p(&["a"]), 1));
}

// ---------------------------------------------------------------- pathContains

#[test]
fn char_path_contains_ascii_case_insensitive() {
    let r = cond_rule("pathContains", json!("Downloads"));
    assert!(matches_path(&r, &p(&["downloads", "a.pdf"]), 1));
    assert!(matches_path(&r, &p(&["DOWNLOADS", "a.pdf"]), 1));
    assert!(!matches_path(&r, &p(&["Documents", "a.pdf"]), 1));

    // Unlike nameMatches, pathContains looks at the whole path string.
    let name_part = cond_rule("pathContains", json!("A.PDF"));
    assert!(matches_path(&name_part, &p(&["x", "a.pdf"]), 1));
}

#[test]
fn char_path_contains_non_ascii_case_sensitive() {
    let r = cond_rule("pathContains", json!("Ă"));
    assert!(matches_path(&r, &p(&["Ă", "a.pdf"]), 1));
    // characterization: current behavior, only ASCII is case-folded; "Ă" does not match "ă"
    assert!(!matches_path(&r, &p(&["ă", "a.pdf"]), 1));

    let lower = cond_rule("pathContains", json!("facturi"));
    assert!(matches_path(&lower, &p(&["FACTURI", "a.pdf"]), 1));
}

#[test]
fn char_path_contains_windows_backslash_string() {
    let path = "C:\\Users\\Vlad\\Downloads\\a.pdf";
    let r = cond_rule("pathContains", json!("\\downloads\\"));
    assert!(matches_str(&r, path, 1));

    let fwd = cond_rule("pathContains", json!("/downloads/"));
    // Plain substring check: separators are not normalized.
    assert!(!matches_str(&fwd, path, 1));
}

#[test]
fn char_path_contains_empty_or_non_string_matches_all() {
    let empty = cond_rule("pathContains", json!(""));
    // characterization: current behavior, an empty needle matches every path
    assert!(matches_path(&empty, &p(&["a.pdf"]), 1));

    let num = cond_rule("pathContains", json!(1));
    // characterization: current behavior, a non-string needle is read as "" and matches every path
    assert!(matches_path(&num, &p(&["a.pdf"]), 1));
}

// ---------------------------------------------------------------- combinators

#[test]
fn char_unknown_condition_type_is_false() {
    let r = cond_rule("mimeIs", json!("application/pdf"));
    // characterization: current behavior, unknown/unimplemented condition types (e.g. mimeIs) evaluate to false silently
    assert!(!matches_path(&r, &p(&["a.pdf"]), 1));

    let typo = cond_rule("Ext", json!("pdf"));
    // Type names are case-sensitive.
    assert!(!matches_path(&typo, &p(&["a.pdf"]), 1));
}

#[test]
fn char_multiple_conditions_are_anded() {
    let r = rule(
        1,
        "and",
        json!([
            { "type": "ext", "value": "pdf" },
            { "type": "sizeGt", "value": 10 },
            { "type": "pathContains", "value": "inbox" }
        ]),
        json!([]),
    );
    assert!(matches_path(&r, &p(&["inbox", "a.pdf"]), 11));
    assert!(!matches_path(&r, &p(&["inbox", "a.txt"]), 11));
    assert!(!matches_path(&r, &p(&["inbox", "a.pdf"]), 10));
    assert!(!matches_path(&r, &p(&["other", "a.pdf"]), 11));

    // One unknown condition makes the whole rule false.
    let with_unknown = rule(
        2,
        "and-unknown",
        json!([
            { "type": "ext", "value": "pdf" },
            { "type": "bogus", "value": true }
        ]),
        json!([]),
    );
    assert!(!matches_path(&with_unknown, &p(&["a.pdf"]), 1));
}

// ---------------------------------------------------------------- matching_rules

fn names(rules: &[&Rule]) -> Vec<String> {
    rules.iter().map(|r| r.name.clone()).collect()
}

#[test]
fn char_matching_rules_preserves_order_and_returns_all() {
    let rules = vec![
        rule(
            1,
            "pdf",
            json!([{ "type": "ext", "value": "pdf" }]),
            json!([]),
        ),
        rule(
            2,
            "txt",
            json!([{ "type": "ext", "value": "txt" }]),
            json!([]),
        ),
        rule(
            3,
            "big",
            json!([{ "type": "sizeGt", "value": 5 }]),
            json!([]),
        ),
        rule(
            4,
            "name",
            json!([{ "type": "nameMatches", "value": "^a" }]),
            json!([]),
        ),
    ];
    let path = p(&["a.pdf"]);
    let s = path.to_string_lossy();
    let got = matching_rules(&rules, &FileFacts { path: &s, size: 10 });
    assert_eq!(names(&got), vec!["pdf", "big", "name"]);

    // Reversed input gives reversed output: order comes from the slice.
    let mut rev = rules.clone();
    rev.reverse();
    let got = matching_rules(&rev, &FileFacts { path: &s, size: 10 });
    assert_eq!(names(&got), vec!["name", "big", "pdf"]);
}

#[test]
fn char_matching_rules_empty_and_none() {
    let path = p(&["a.pdf"]);
    let s = path.to_string_lossy();
    let facts = FileFacts { path: &s, size: 1 };
    assert!(matching_rules(&[], &facts).is_empty());

    let rules = vec![rule(
        1,
        "txt",
        json!([{ "type": "ext", "value": "txt" }]),
        json!([]),
    )];
    assert!(matching_rules(&rules, &facts).is_empty());
}

#[test]
fn char_matching_rules_skips_disabled() {
    let mut off = rule(1, "off", json!([]), json!([]));
    off.enabled = false;
    let on = rule(2, "on", json!([]), json!([]));
    let rules = vec![off, on];
    let path = p(&["a.pdf"]);
    let s = path.to_string_lossy();
    let got = matching_rules(&rules, &FileFacts { path: &s, size: 1 });
    assert_eq!(names(&got), vec!["on"]);
}
