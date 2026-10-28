//! Property tests for `planner::plan` on generated trees and rule lists
//! (PLAN.md 4.6 "property"): one op per source, destinations unique
//! case-insensitively and free on disk, destination folder equals the
//! expanded template, the first matching rule wins, and the plan is
//! deterministic.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use proptest::prelude::*;
use regex::Regex;
use serde_json::json;
use uuid::Uuid;
use valet_core::planner::{OpKind, Plan, PlanContext, SkipReason, plan};
use valet_core::platform::testing::FakePlatform;
use valet_core::rules::Rule;

/// Source sub-folders (distinct case-insensitively, so the tree is the same
/// on case-sensitive and case-insensitive file systems).
const DIRS: &[&str] = &["", "a", "b", "a/c", "Deep/er/x"];
/// Stems with case variants of each other.
const STEMS: &[&str] = &[
    "report",
    "Report",
    "REPORT",
    "photo",
    "Photo_2",
    "factura_1",
    "Factura_1",
    "notes",
    "a",
    "x.y",
];
/// Extensions with case variants; "" means no extension.
const EXTS: &[&str] = &["pdf", "PDF", "Pdf", "jpg", "JPG", "txt", "tar.gz", ""];
/// Folders (relative to the scan root) that get pre-existing files; most of
/// them are also expansions of the templates below.
const PRE_DIRS: &[&str] = &[
    "Docs",
    "DOCS",
    "Media/pdf",
    "Media/jpg",
    "pdf",
    "jpg",
    "txt",
    "gz",
    "a",
];
/// Destination folder templates. `ABS/` is replaced by an absolute folder
/// outside the scan root.
const TEMPLATES: &[&str] = &[
    "Docs",
    "docs",
    "Media/{ext}",
    "{ext}",
    "a",
    "ABS/{ext}",
    "ABS/Flat",
];
const EXT_VALUES: &[&str] = &["pdf", "PDF", "jpg", "txt", "gz"];
const NAME_PATTERNS: &[&str] = &["(?i)^report", "^photo", "factura", "\\d", "."];

#[derive(Debug, Clone)]
struct GenRule {
    by_ext: bool,
    value: usize,
    copy: bool,
    template: usize,
    enabled: bool,
}

#[derive(Debug, Clone)]
struct Case {
    files: Vec<(usize, usize, usize)>,
    pre: Vec<(usize, usize, usize)>,
    rules: Vec<GenRule>,
}

fn case() -> impl Strategy<Value = Case> {
    let file = (0..DIRS.len(), 0..STEMS.len(), 0..EXTS.len());
    let pre = (0..PRE_DIRS.len(), 0..STEMS.len(), 0..EXTS.len());
    let rule = (
        any::<bool>(),
        0usize..5,
        prop::bool::weighted(0.25),
        0..TEMPLATES.len(),
        prop::bool::weighted(0.9),
    )
        .prop_map(|(by_ext, value, copy, template, enabled)| GenRule {
            by_ext,
            value,
            copy,
            template,
            enabled,
        });
    (
        prop::collection::vec(file, 1..30),
        prop::collection::vec(pre, 0..10),
        prop::collection::vec(rule, 1..6),
    )
        .prop_map(|(files, pre, rules)| Case { files, pre, rules })
}

fn file_name(stem: usize, ext: usize) -> String {
    match EXTS[ext] {
        "" => STEMS[stem].to_owned(),
        e => format!("{}.{e}", STEMS[stem]),
    }
}

fn fold(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

/// Writes `rel` under `root` unless a case-insensitively equal path was
/// already written. Contents are unique per file.
fn put(root: &Path, rel: &str, written: &mut HashSet<String>, n: usize) -> Option<PathBuf> {
    let p = root.join(rel);
    if !written.insert(fold(&p)) {
        return None;
    }
    // A generated file can clash with a generated folder of the same name
    // (`a` vs `a/c`): the later one is left out, on every OS alike.
    fs::create_dir_all(p.parent().unwrap()).ok()?;
    if fs::symlink_metadata(&p).is_ok() {
        return None;
    }
    fs::write(&p, format!("file {n} {rel}")).ok()?;
    Some(p)
}

struct Built {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    abs: PathBuf,
    rules: Vec<Rule>,
    templates: HashMap<Uuid, String>,
    sources: Vec<PathBuf>,
    pre: Vec<PathBuf>,
}

fn build(c: &Case) -> Built {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("src");
    let abs = tmp.path().join("out");
    fs::create_dir_all(&root).unwrap();
    let mut written = HashSet::new();
    let mut sources = Vec::new();
    for (i, &(d, s, e)) in c.files.iter().enumerate() {
        let rel = match DIRS[d] {
            "" => file_name(s, e),
            dir => format!("{dir}/{}", file_name(s, e)),
        };
        sources.extend(put(&root, &rel, &mut written, i));
    }
    let mut pre = Vec::new();
    for (i, &(d, s, e)) in c.pre.iter().enumerate() {
        let rel = format!("{}/{}", PRE_DIRS[d], file_name(s, e));
        pre.extend(put(&root, &rel, &mut written, 1000 + i));
        // Same name also under the absolute destination root.
        let abs_rel = format!(
            "{}/{}",
            if i.is_multiple_of(2) { "Flat" } else { "pdf" },
            file_name(s, e)
        );
        pre.extend(put(&abs, &abs_rel, &mut written, 2000 + i));
    }
    let mut rules = Vec::new();
    let mut templates = HashMap::new();
    for (i, r) in c.rules.iter().enumerate() {
        let id = Uuid::from_u128(i as u128 + 1);
        let template = TEMPLATES[r.template].replace("ABS", &abs.to_string_lossy());
        let cond = if r.by_ext {
            json!({ "type": "ext", "value": EXT_VALUES[r.value] })
        } else {
            json!({ "type": "nameMatches", "value": NAME_PATTERNS[r.value] })
        };
        let action = if r.copy { "copyTo" } else { "moveTo" };
        rules.push(
            serde_json::from_value::<Rule>(json!({
                "id": id,
                "name": format!("r{i}"),
                "enabled": r.enabled,
                "conditions": [cond],
                "actions": [{ "type": action, "params": { "path": template } }],
            }))
            .unwrap(),
        );
        templates.insert(id, template);
    }
    Built {
        _tmp: tmp,
        root,
        abs,
        rules,
        templates,
        sources,
        pre,
    }
}

fn ctx() -> PlanContext {
    PlanContext {
        // Every fixture file is older than the being-written window.
        now: SystemTime::now() + Duration::from_secs(3600),
        home_dir: None,
        env: Box::new(|_| None),
    }
}

fn last_ext_lower(p: &Path) -> Option<String> {
    p.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| !e.is_empty())
}

/// Independent re-statement of the matcher for the two condition types used.
fn matches(rule: &Rule, file: &Path) -> bool {
    if !rule.enabled {
        return false;
    }
    let name = file.file_name().unwrap().to_str().unwrap();
    rule.conditions.iter().all(|c| {
        let v = c.value.as_str().unwrap();
        match c.r#type.as_str() {
            "ext" => file
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(v)),
            "nameMatches" => Regex::new(v).unwrap().is_match(name),
            other => panic!("unexpected condition {other}"),
        }
    })
}

/// Expected destination folder for `source` under `template` (relative
/// templates resolve against the scan root).
fn expected_folder(root: &Path, template: &str, source: &Path) -> Option<PathBuf> {
    let expanded = if template.contains("{ext}") {
        template.replace("{ext}", &last_ext_lower(source)?)
    } else {
        template.to_owned()
    };
    let p = PathBuf::from(expanded);
    Some(if p.is_absolute() { p } else { root.join(p) })
}

fn check(c: &Case) -> Result<(), TestCaseError> {
    let b = build(c);
    let platform = FakePlatform::default();
    let roots = vec![b.root.clone()];
    let p1: Plan = plan(&roots, &b.rules, &platform, &ctx());
    let p2: Plan = plan(&roots, &b.rules, &platform, &ctx());
    prop_assert_eq!(&p1, &p2, "plan is not deterministic");

    // No source in more than one op.
    let mut srcs = HashSet::new();
    for op in &p1.ops {
        prop_assert!(
            srcs.insert(op.source.clone()),
            "{} planned twice",
            op.source.display()
        );
    }

    // Destinations unique case-insensitively, not on disk, not inside the
    // scanned files, in the expanded template folder.
    let mut dests = HashSet::new();
    for op in &p1.ops {
        prop_assert!(
            dests.insert(fold(&op.dest)),
            "dest {} reused (case-insensitive)",
            op.dest.display()
        );
        prop_assert!(
            fs::symlink_metadata(&op.dest).is_err(),
            "dest {} exists on disk",
            op.dest.display()
        );
        let template = &b.templates[&op.rule_id];
        let want = expected_folder(&b.root, template, &op.source);
        prop_assert!(
            want.is_some(),
            "op planned although {{ext}} cannot expand: {:?}",
            op
        );
        prop_assert_eq!(op.dest.parent(), want.as_deref(), "template {}", template);
        prop_assert_eq!(op.dest.extension(), op.source.extension());
        let rule = b.rules.iter().find(|r| r.id == op.rule_id).unwrap();
        let want_kind = if rule.actions[0].r#type == "copyTo" {
            OpKind::Copy
        } else {
            OpKind::Move
        };
        prop_assert_eq!(op.kind, want_kind);
    }
    // Copies keep the source, so the copy dest must also differ from every
    // source; moves too (covered by "not on disk", restated for clarity).
    for op in &p1.ops {
        prop_assert!(!srcs.contains(&op.dest));
    }

    // Every file on disk: the op (if any) uses the first matching rule; a
    // file with a matching rule but no op has a skip explaining why.
    let all: Vec<&PathBuf> = b
        .sources
        .iter()
        .chain(b.pre.iter())
        .filter(|p| p.starts_with(&b.root))
        .collect();
    for f in all {
        let first = b.rules.iter().find(|r| matches(r, f));
        let op = p1.ops.iter().find(|o| &o.source == f);
        match (first, op) {
            (Some(r), Some(op)) => prop_assert_eq!(op.rule_id, r.id, "{}", f.display()),
            (None, Some(op)) => prop_assert!(
                false,
                "{} planned without a matching rule: {:?}",
                f.display(),
                op
            ),
            (Some(r), None) => {
                let reasons: Vec<SkipReason> = p1
                    .skipped
                    .iter()
                    .filter(|s| &s.path == f && s.rule_name.as_deref() == Some(r.name.as_str()))
                    .map(|s| s.reason)
                    .collect();
                prop_assert!(
                    reasons
                        .iter()
                        .any(|r| matches!(r, SkipReason::NoExtension | SkipReason::AlreadyInPlace)),
                    "{} matched {} but has no op; skips {:?}",
                    f.display(),
                    r.name,
                    reasons
                );
            }
            (None, None) => {}
        }
    }
    // Nothing outside the scan root is planned as a source.
    for op in &p1.ops {
        prop_assert!(op.source.starts_with(&b.root));
        prop_assert!(!op.source.starts_with(&b.abs));
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn plan_invariants(c in case()) {
        check(&c)?;
    }
}

/// Fixed case: three case-variant names into one folder, plus a
/// pre-existing file there; every dest distinct and free.
#[test]
fn case_variants_into_one_folder() {
    let c = Case {
        files: vec![(1, 0, 0), (2, 1, 1), (3, 2, 2)],
        pre: vec![(0, 0, 0)],
        rules: vec![GenRule {
            by_ext: true,
            value: 0,
            copy: false,
            template: 1, // "docs"
            enabled: true,
        }],
    };
    check(&c).unwrap();
    let b = build(&c);
    let p = plan(
        std::slice::from_ref(&b.root),
        &b.rules,
        &FakePlatform::default(),
        &ctx(),
    );
    assert_eq!(p.ops.len(), 3, "{p:?}");
}
