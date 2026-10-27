//! Characterization tests for `valet_core::engine::dry_run_for_paths`.
//!
//! These pin the behavior as it is today (Phase 0), including behavior that
//! looks wrong. Assertions marked `// characterization: current behavior, ...`
//! describe a suspected bug or invariant gap; change them deliberately when
//! the planner is rewritten.
//!
//! WalkDir is used without `sort_by`, so the order of files inside one
//! directory follows the OS directory listing and is not guaranteed. Tests
//! compare sorted rows unless they pin an order the code does guarantee
//! (roots in argument order, rules in DB rowid order, actions in rule order).

mod common;

use std::path::{Path, PathBuf};

use common::{rule, s, write_file};
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;
use valet_core::engine::dry_run_for_paths;
use valet_core::model::{DryRunPlan, Op};
use valet_core::rules::Rule;
use valet_core::storage::Db;

struct Env {
    tmp: TempDir,
    db: Db,
}

impl Env {
    async fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = Db::connect(&tmp.path().join("t.sqlite3"))
            .await
            .expect("db connect + migrations");
        Self { tmp, db }
    }

    /// Scan root, separate from the database file.
    fn root(&self) -> PathBuf {
        let r = self.tmp.path().join("root");
        std::fs::create_dir_all(&r).expect("create root");
        r
    }

    async fn add(&self, r: &Rule) {
        self.db.upsert_rule(r).await.expect("upsert rule");
    }

    async fn plan(&self, roots: &[PathBuf]) -> DryRunPlan {
        dry_run_for_paths(roots, &self.db).await.expect("dry run")
    }

    async fn close(self) {
        // Close the pool before the tempdir is dropped (Windows file locks).
        self.db.pool.close().await;
    }
}

type Row = (String, String, String);

/// (file_path, op debug string, rule_name), in plan order.
fn rows(plan: &DryRunPlan) -> Vec<Row> {
    plan.actions
        .iter()
        .map(|a| {
            (
                a.file_path.clone(),
                format!("{:?}", a.op),
                a.rule_name.clone(),
            )
        })
        .collect()
}

fn sorted_rows(plan: &DryRunPlan) -> Vec<Row> {
    let mut r = rows(plan);
    r.sort();
    r
}

fn sorted_paths(plan: &DryRunPlan) -> Vec<String> {
    let mut v: Vec<String> = plan.actions.iter().map(|a| a.file_path.clone()).collect();
    v.sort();
    v
}

fn sorted_strings(paths: &[&Path]) -> Vec<String> {
    let mut v: Vec<String> = paths.iter().map(|p| s(p)).collect();
    v.sort();
    v
}

fn tag_rule(n: u128, name: &str, conditions: serde_json::Value) -> Rule {
    rule(
        n,
        name,
        conditions,
        json!([{ "type": "tag", "params": { "tags": [name] } }]),
    )
}

fn all_files_rule(n: u128, name: &str) -> Rule {
    let mut r = tag_rule(n, name, json!([]));
    r.always_apply = true;
    r
}

// ---------------------------------------------------------------- empty inputs

#[tokio::test]
async fn char_dry_run_no_rules_gives_empty_plan() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    write_file(&root.join("sub").join("b.txt"), 3);

    let plan = env.plan(&[root]).await;
    assert!(plan.actions.is_empty());
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_empty_directory_gives_empty_plan() {
    let env = Env::new().await;
    env.add(&all_files_rule(1, "all")).await;
    let root = env.root();
    std::fs::create_dir_all(root.join("empty_sub")).unwrap();

    let plan = env.plan(&[root]).await;
    assert!(plan.actions.is_empty());
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_no_roots_gives_empty_plan() {
    let env = Env::new().await;
    env.add(&all_files_rule(1, "all")).await;
    let plan = env.plan(&[]).await;
    assert!(plan.actions.is_empty());
    env.close().await;
}

// ---------------------------------------------------------------- action mapping

#[tokio::test]
async fn char_dry_run_move_to_destination_is_raw_param() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    write_file(&root.join("b.txt"), 3);
    env.add(&rule(
        1,
        "pdfs",
        json!([{ "type": "ext", "value": "pdf" }]),
        json!([{ "type": "moveTo", "params": { "path": "~/Documents/PDF" } }]),
    ))
    .await;

    let plan = env.plan(std::slice::from_ref(&root)).await;
    assert_eq!(plan.actions.len(), 1);
    let a = &plan.actions[0];
    // file_path is the walked path (root joined with the name), lossy UTF-8.
    assert_eq!(a.file_path, s(&root.join("a.pdf")));
    assert_eq!(a.rule_id, Uuid::from_u128(1));
    assert_eq!(a.rule_name, "pdfs");
    match &a.op {
        Op::MoveTo { path } => {
            // characterization: current behavior, MoveTo.path is the raw rule param: not joined with the file name, "~" not expanded, no collision-free final name computed
            assert_eq!(path, "~/Documents/PDF");
        }
        other => panic!("expected MoveTo, got {other:?}"),
    }
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_move_to_relative_destination_is_raw_param() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    env.add(&rule(
        1,
        "rel",
        json!([{ "type": "ext", "value": "pdf" }]),
        json!([{ "type": "moveTo", "params": { "path": "Sorted" } }]),
    ))
    .await;

    let plan = env.plan(&[root]).await;
    assert_eq!(plan.actions.len(), 1);
    // characterization: current behavior, a relative destination is passed through unresolved (relative to nothing in particular)
    assert!(matches!(&plan.actions[0].op, Op::MoveTo { path } if path == "Sorted"));
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_action_types_map_to_ops_in_order() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    env.add(&rule(
        1,
        "multi",
        json!([{ "type": "ext", "value": "pdf" }]),
        json!([
            { "type": "copyTo", "params": { "path": "C:\\Backup" } },
            { "type": "tag", "params": { "tags": ["work", 1, null, "2024"] } },
            { "type": "rename", "params": { "pattern": "{date}-{name}" } },
            { "type": "quarantine" }
        ]),
    ))
    .await;

    let plan = env.plan(&[root]).await;
    assert_eq!(plan.actions.len(), 4);
    assert!(matches!(&plan.actions[0].op, Op::CopyTo { path } if path == "C:\\Backup"));
    match &plan.actions[1].op {
        // Non-string tags are dropped silently.
        Op::Tag { tags } => assert_eq!(tags, &vec!["work".to_string(), "2024".to_string()]),
        other => panic!("expected Tag, got {other:?}"),
    }
    assert!(matches!(&plan.actions[2].op, Op::Rename { pattern } if pattern == "{date}-{name}"));
    assert!(matches!(&plan.actions[3].op, Op::Quarantine));
    assert!(plan.actions.iter().all(|a| a.rule_name == "multi"));
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_unknown_or_incomplete_actions_are_dropped() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    env.add(&rule(
        1,
        "broken",
        json!([{ "type": "ext", "value": "pdf" }]),
        json!([
            { "type": "delete" },
            { "type": "MoveTo", "params": { "path": "X" } },
            { "type": "moveTo" },
            { "type": "moveTo", "params": { "path": 5 } },
            { "type": "copyTo", "params": { "dest": "X" } },
            { "type": "tag", "params": { "tags": "single" } },
            { "type": "rename" },
            { "type": "quarantine", "params": { "ignored": true } }
        ]),
    ))
    .await;

    let plan = env.plan(&[root]).await;
    // characterization: current behavior, unknown action types and actions with a missing/wrong-typed param are dropped silently (no error, no warning in the plan); only quarantine survives here
    assert_eq!(plan.actions.len(), 1);
    assert!(matches!(&plan.actions[0].op, Op::Quarantine));
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_rule_with_two_actions_gives_two_entries() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    env.add(&rule(
        1,
        "two",
        json!([{ "type": "ext", "value": "pdf" }]),
        json!([
            { "type": "moveTo", "params": { "path": "D" } },
            { "type": "tag", "params": { "tags": ["t"] } }
        ]),
    ))
    .await;

    let plan = env.plan(std::slice::from_ref(&root)).await;
    let f = s(&root.join("a.pdf"));
    assert_eq!(
        rows(&plan),
        vec![
            (
                f.clone(),
                "MoveTo { path: \"D\" }".to_string(),
                "two".to_string()
            ),
            (f, "Tag { tags: [\"t\"] }".to_string(), "two".to_string()),
        ]
    );
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_file_matching_two_rules_gets_both() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    let r1 = rule(
        1,
        "first",
        json!([{ "type": "ext", "value": "pdf" }]),
        json!([{ "type": "moveTo", "params": { "path": "A" } }]),
    );
    let r2 = rule(
        2,
        "second",
        json!([{ "type": "nameMatches", "value": "^a" }]),
        json!([{ "type": "moveTo", "params": { "path": "B" } }]),
    );
    env.add(&r1).await;
    env.add(&r2).await;
    // Re-upserting rule 1 keeps its rowid, so it stays first.
    env.add(&r1).await;

    let plan = env.plan(&[root]).await;
    let got: Vec<(String, String)> = rows(&plan)
        .into_iter()
        .map(|(_, op, name)| (name, op))
        .collect();
    // characterization: current behavior, one file gets two conflicting MoveTo ops (A and B); no first-match-wins or conflict detection. Order = rules table rowid order
    assert_eq!(
        got,
        vec![
            ("first".to_string(), "MoveTo { path: \"A\" }".to_string()),
            ("second".to_string(), "MoveTo { path: \"B\" }".to_string()),
        ]
    );
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_disabled_rule_in_db_is_ignored() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    let mut r = all_files_rule(1, "off");
    r.enabled = false;
    env.add(&r).await;

    let plan = env.plan(&[root]).await;
    assert!(plan.actions.is_empty());
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_empty_conditions_rule_plans_every_file() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    write_file(&root.join("sub").join("b.txt"), 0);
    env.add(&tag_rule(1, "noconds", json!([]))).await;

    let plan = env.plan(std::slice::from_ref(&root)).await;
    // characterization: current behavior, an enabled rule without conditions (and without alwaysApply) plans an action for every file in scope
    assert_eq!(
        sorted_paths(&plan),
        sorted_strings(&[&root.join("a.pdf"), &root.join("sub").join("b.txt")])
    );
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_size_comes_from_file_metadata() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("five.bin"), 5);
    write_file(&root.join("six.bin"), 6);
    env.add(&tag_rule(
        1,
        "gt5",
        json!([{ "type": "sizeGt", "value": 5 }]),
    ))
    .await;

    let plan = env.plan(std::slice::from_ref(&root)).await;
    assert_eq!(sorted_paths(&plan), vec![s(&root.join("six.bin"))]);
    env.close().await;
}

// ---------------------------------------------------------------- walking

#[tokio::test]
async fn char_dry_run_recurses_into_subdirectories() {
    let env = Env::new().await;
    let root = env.root();
    let a = root.join("a.pdf");
    let b = root.join("sub").join("b.pdf");
    let c = root.join("sub").join("deeper").join("c.pdf");
    for f in [&a, &b, &c] {
        write_file(f, 1);
    }
    env.add(&tag_rule(
        1,
        "pdf",
        json!([{ "type": "ext", "value": "pdf" }]),
    ))
    .await;

    let plan = env.plan(std::slice::from_ref(&root)).await;
    assert_eq!(sorted_paths(&plan), sorted_strings(&[&a, &b, &c]));
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_does_not_exclude_hidden_or_default_excluded_dirs() {
    let env = Env::new().await;
    let root = env.root();
    let files = [
        root.join("visible.pdf"),
        root.join(".hidden.pdf"),
        root.join(".git").join("objects").join("x.pdf"),
        root.join("node_modules").join("pkg").join("y.pdf"),
        root.join("target").join("z.pdf"),
        root.join(".valetignore"),
    ];
    for f in &files {
        write_file(f, 1);
    }
    env.add(&all_files_rule(1, "all")).await;

    let plan = env.plan(std::slice::from_ref(&root)).await;
    let refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
    // characterization: current behavior, dot files and .git/node_modules/target are walked and planned; default exclusions (CLAUDE.md invariant 5) are not applied, .valetignore is not honored (known Phase 1 gap)
    assert_eq!(sorted_paths(&plan), sorted_strings(&refs));
    env.close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn char_dry_run_does_not_follow_symlinks() {
    use std::os::unix::fs::symlink;

    let env = Env::new().await;
    let root = env.root();
    let outside = env.tmp.path().join("outside");
    write_file(&outside.join("real.pdf"), 1);
    write_file(&outside.join("dir").join("inner.pdf"), 1);

    let real_in_root = root.join("local.pdf");
    write_file(&real_in_root, 1);
    symlink(outside.join("real.pdf"), root.join("link.pdf")).unwrap();
    symlink(outside.join("dir"), root.join("linkdir")).unwrap();
    env.add(&all_files_rule(1, "all")).await;

    let plan = env.plan(std::slice::from_ref(&root)).await;
    // Symlinked files are skipped (not planned), symlinked dirs are not descended.
    assert_eq!(sorted_paths(&plan), vec![s(&real_in_root)]);
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_multiple_roots_in_argument_order() {
    let env = Env::new().await;
    let base = env.root();
    let r1 = base.join("one");
    let r2 = base.join("two");
    write_file(&r1.join("a.pdf"), 1);
    write_file(&r1.join("b.pdf"), 1);
    write_file(&r2.join("c.pdf"), 1);
    env.add(&all_files_rule(1, "all")).await;

    let plan = env.plan(&[r2.clone(), r1.clone()]).await;
    let paths: Vec<String> = plan.actions.iter().map(|a| a.file_path.clone()).collect();
    assert_eq!(paths.len(), 3);
    // All of the first root comes before the second root.
    assert_eq!(paths[0], s(&r2.join("c.pdf")));
    let mut rest = paths[1..].to_vec();
    rest.sort();
    assert_eq!(
        rest,
        sorted_strings(&[&r1.join("a.pdf"), &r1.join("b.pdf")])
    );
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_overlapping_roots_duplicate_actions() {
    let env = Env::new().await;
    let root = env.root();
    let inner = root.join("sub");
    let f = inner.join("a.pdf");
    write_file(&f, 1);
    env.add(&all_files_rule(1, "all")).await;

    let plan = env.plan(&[root.clone(), inner.clone()]).await;
    // characterization: current behavior, overlapping roots are not deduplicated; the same file is planned once per root that contains it
    assert_eq!(sorted_paths(&plan), vec![s(&f), s(&f)]);

    // Same root passed twice: also duplicated.
    let plan = env.plan(&[root.clone(), root]).await;
    // characterization: current behavior, a repeated root plans every file twice
    assert_eq!(plan.actions.len(), 2);
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_nonexistent_root_is_ok_and_empty() {
    let env = Env::new().await;
    env.add(&all_files_rule(1, "all")).await;
    let root = env.root();
    let f = root.join("a.pdf");
    write_file(&f, 1);
    let missing = env.tmp.path().join("does-not-exist");

    let res = dry_run_for_paths(std::slice::from_ref(&missing), &env.db).await;
    // characterization: current behavior, a missing root is not an error; WalkDir errors are dropped by filter_map(ok) and the plan is empty
    let plan = res.expect("missing root returns Ok");
    assert!(plan.actions.is_empty());

    // Mixed with a real root, the real root is still scanned.
    let plan = env.plan(&[missing, root]).await;
    assert_eq!(sorted_paths(&plan), vec![s(&f)]);
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_root_that_is_a_file_plans_that_file() {
    let env = Env::new().await;
    env.add(&all_files_rule(1, "all")).await;
    let root = env.root();
    let f = root.join("single.pdf");
    write_file(&f, 1);

    let plan = env.plan(std::slice::from_ref(&f)).await;
    assert_eq!(sorted_paths(&plan), vec![s(&f)]);
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_destination_inside_root_is_not_excluded() {
    let env = Env::new().await;
    let root = env.root();
    let dest = root.join("Sorted");
    let already = dest.join("old.pdf");
    let fresh = root.join("new.pdf");
    write_file(&already, 1);
    write_file(&fresh, 1);
    env.add(&rule(
        1,
        "sort",
        json!([{ "type": "ext", "value": "pdf" }]),
        json!([{ "type": "moveTo", "params": { "path": s(&dest) } }]),
    ))
    .await;

    let plan = env.plan(std::slice::from_ref(&root)).await;
    // characterization: current behavior, a file already in the destination folder is planned to move into that same folder again (no destination exclusion / no-op detection)
    assert_eq!(sorted_paths(&plan), sorted_strings(&[&already, &fresh]));
    env.close().await;
}

// ---------------------------------------------------------------- determinism

#[tokio::test]
async fn char_dry_run_is_deterministic_when_sorted() {
    let env = Env::new().await;
    let root = env.root();
    for name in ["c.pdf", "a.pdf", "b.txt", "Z.PDF", "d.jpg"] {
        write_file(&root.join(name), 2);
    }
    for name in ["x.pdf", "y.txt"] {
        write_file(&root.join("sub").join(name), 20);
    }
    env.add(&rule(
        1,
        "pdf",
        json!([{ "type": "ext", "value": "pdf" }]),
        json!([
            { "type": "moveTo", "params": { "path": "P" } },
            { "type": "tag", "params": { "tags": ["pdf"] } }
        ]),
    ))
    .await;
    env.add(&tag_rule(
        2,
        "big",
        json!([{ "type": "sizeGt", "value": 10 }]),
    ))
    .await;

    let first = env.plan(std::slice::from_ref(&root)).await;
    let second = env.plan(std::slice::from_ref(&root)).await;
    // pdf rule: 4 files x 2 actions; big rule: 2 files in sub.
    assert_eq!(first.actions.len(), 10);
    // Plan order depends on OS directory listing order (WalkDir has no
    // sort_by), so only the sorted rows are compared.
    assert_eq!(sorted_rows(&first), sorted_rows(&second));
    env.close().await;
}
