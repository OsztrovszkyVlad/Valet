//! Characterization tests for the dry run, written in Phase 0 against
//! `engine::dry_run_for_paths` and ported in Phase 1 to `planner::plan`.
//!
//! Assertions marked `// characterization: current behavior, ...` describe
//! behavior that still holds and may be a gap. Assertions marked
//! `// Phase 1: changed intentionally: <old> -> <new>` record where the
//! planner deliberately differs from the Phase 0 engine.
//!
//! Phase 0 walked without `sort_by`; the planner sorts entries by file name.
//! Tests that compare sorted rows keep doing so.
//!
//! Phase 1: `tag` actions are not planned until Phase 2, so the rules used to
//! observe walking and matching move files to the relative folder `Out`
//! instead of tagging them.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use common::{rule, s, write_file};
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;
use valet_core::planner::{OpKind, Plan, PlanContext, SkipReason, plan};
use valet_core::platform::testing::FakePlatform;
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

    fn home(&self) -> PathBuf {
        self.tmp.path().join("home")
    }

    async fn plan(&self, roots: &[PathBuf]) -> Plan {
        let rules = self.db.list_rules().await.expect("list rules");
        let ctx = PlanContext {
            // One hour ahead so fresh fixtures are outside the being-written window.
            now: SystemTime::now() + Duration::from_secs(3600),
            home_dir: Some(self.home()),
            env: Box::new(|_| None),
        };
        plan(roots, &rules, &FakePlatform::default(), &ctx)
    }

    async fn close(self) {
        // Close the pool before the tempdir is dropped (Windows file locks).
        self.db.pool.close().await;
    }
}

type Row = (String, String, String);

/// (source, "<kind> <dest>", rule_name), in plan order.
fn rows(plan: &Plan) -> Vec<Row> {
    plan.ops
        .iter()
        .map(|o| {
            (
                s(&o.source),
                format!("{:?} {}", o.kind, s(&o.dest)),
                o.rule_name.clone(),
            )
        })
        .collect()
}

fn skips(plan: &Plan) -> Vec<(String, SkipReason, Option<String>)> {
    plan.skipped
        .iter()
        .map(|k| (s(&k.path), k.reason, k.rule_name.clone()))
        .collect()
}

fn sorted_rows(plan: &Plan) -> Vec<Row> {
    let mut r = rows(plan);
    r.sort();
    r
}

fn sorted_paths(plan: &Plan) -> Vec<String> {
    let mut v: Vec<String> = plan.ops.iter().map(|o| s(&o.source)).collect();
    v.sort();
    v
}

fn sorted_strings(paths: &[&Path]) -> Vec<String> {
    let mut v: Vec<String> = paths.iter().map(|p| s(p)).collect();
    v.sort();
    v
}

/// Phase 1: was `tag_rule` (a `tag` action); moves to `Out` now, see the module doc.
fn move_rule(n: u128, name: &str, conditions: serde_json::Value) -> Rule {
    rule(
        n,
        name,
        conditions,
        json!([{ "type": "moveTo", "params": { "path": "Out" } }]),
    )
}

fn all_files_rule(n: u128, name: &str) -> Rule {
    let mut r = move_rule(n, name, json!([]));
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
    assert!(plan.ops.is_empty());
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_empty_directory_gives_empty_plan() {
    let env = Env::new().await;
    env.add(&all_files_rule(1, "all")).await;
    let root = env.root();
    std::fs::create_dir_all(root.join("empty_sub")).unwrap();

    let plan = env.plan(&[root]).await;
    assert!(plan.ops.is_empty());
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_no_roots_gives_empty_plan() {
    let env = Env::new().await;
    env.add(&all_files_rule(1, "all")).await;
    let plan = env.plan(&[]).await;
    assert!(plan.ops.is_empty());
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
    assert_eq!(plan.ops.len(), 1);
    let a = &plan.ops[0];
    // source is the walked path (root joined with the name).
    assert_eq!(s(&a.source), s(&root.join("a.pdf")));
    assert_eq!(a.rule_id, Uuid::from_u128(1));
    assert_eq!(a.rule_name, "pdfs");
    assert_eq!(a.kind, OpKind::Move);
    // Phase 1: changed intentionally: MoveTo.path was the raw rule param "~/Documents/PDF" (not joined with the file name, "~" not expanded, no collision-free name) -> dest is the full file path: "~" expanded to the home dir, folder joined with the file name, collision-free
    assert_eq!(
        a.dest,
        env.home().join("Documents").join("PDF").join("a.pdf")
    );
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

    let plan = env.plan(std::slice::from_ref(&root)).await;
    assert_eq!(plan.ops.len(), 1);
    // Phase 1: changed intentionally: a relative destination was passed through unresolved ("Sorted") -> it resolves against the scan root the file was found under
    assert_eq!(plan.ops[0].dest, root.join("Sorted").join("a.pdf"));
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

    let plan = env.plan(std::slice::from_ref(&root)).await;
    // Phase 1: changed intentionally: four entries (CopyTo, Tag, Rename, Quarantine) -> one Copy op; tag/rename/quarantine return in Phase 2 and are not reported when a copy or move is planned for the file
    assert_eq!(plan.ops.len(), 1);
    assert!(plan.skipped.is_empty());
    let backup = Path::new("C:\\Backup");
    // Absolute on Windows; on other systems a relative folder under the root.
    let folder = if backup.is_absolute() {
        backup.to_path_buf()
    } else {
        root.join(backup)
    };
    assert_eq!(plan.ops[0].kind, OpKind::Copy);
    assert_eq!(plan.ops[0].dest, folder.join("a.pdf"));
    assert!(plan.ops.iter().all(|a| a.rule_name == "multi"));
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

    let plan = env.plan(std::slice::from_ref(&root)).await;
    // Phase 1: changed intentionally: unknown or incomplete actions were dropped silently and quarantine survived -> nothing is planned; the first moveTo (no path param) is reported as InvalidDestination and the rule's other actions as UnsupportedAction
    assert!(plan.ops.is_empty());
    let f = s(&root.join("a.pdf"));
    assert_eq!(
        skips(&plan),
        vec![
            (
                f.clone(),
                SkipReason::InvalidDestination,
                Some("broken".to_string())
            ),
            (f, SkipReason::UnsupportedAction, Some("broken".to_string())),
        ]
    );
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
    // Phase 1: changed intentionally: two entries (MoveTo "D", Tag) -> one Move op to root/D/a.pdf; the tag is not planned until Phase 2 and not reported because a move was planned
    assert_eq!(
        rows(&plan),
        vec![(
            f,
            format!("Move {}", s(&root.join("D").join("a.pdf"))),
            "two".to_string()
        )]
    );
    assert!(plan.skipped.is_empty());
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

    let plan = env.plan(std::slice::from_ref(&root)).await;
    let got: Vec<(String, String)> = rows(&plan)
        .into_iter()
        .map(|(_, op, name)| (name, op))
        .collect();
    // Phase 1: changed intentionally: one file got two conflicting MoveTo ops (A and B) -> first match wins in rules table rowid order (one Move to A); the second rule is reported as Conflict
    assert_eq!(
        got,
        vec![(
            "first".to_string(),
            format!("Move {}", s(&root.join("A").join("a.pdf")))
        )]
    );
    assert_eq!(
        skips(&plan),
        vec![(
            s(&root.join("a.pdf")),
            SkipReason::Conflict,
            Some("second".to_string())
        )]
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
    assert!(plan.ops.is_empty());
    env.close().await;
}

#[tokio::test]
async fn char_dry_run_empty_conditions_rule_plans_every_file() {
    let env = Env::new().await;
    let root = env.root();
    write_file(&root.join("a.pdf"), 3);
    write_file(&root.join("sub").join("b.txt"), 0);
    env.add(&move_rule(1, "noconds", json!([]))).await;

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
    env.add(&move_rule(
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
    env.add(&move_rule(
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
    // Phase 1: changed intentionally: every file above was planned (no default exclusions, .valetignore ignored) -> the root contains .valetignore, so the whole root is one Excluded entry and nothing is planned
    assert!(plan.ops.is_empty());
    assert_eq!(skips(&plan), vec![(s(&root), SkipReason::Excluded, None)]);

    // Same tree without .valetignore (second root).
    let root2 = env.tmp.path().join("root2");
    for f in &files[..5] {
        write_file(&root2.join(f.strip_prefix(&root).unwrap()), 1);
    }
    let plan = env.plan(std::slice::from_ref(&root2)).await;
    // Phase 1: changed intentionally: dot files and .git/node_modules/target were walked and planned -> only visible.pdf is planned; each excluded dir is one Excluded entry, the dot file is Hidden
    assert_eq!(sorted_paths(&plan), vec![s(&root2.join("visible.pdf"))]);
    assert_eq!(
        skips(&plan),
        vec![
            (s(&root2.join(".git")), SkipReason::Excluded, None),
            (
                s(&root2.join(".hidden.pdf")),
                SkipReason::Hidden,
                Some("all".to_string())
            ),
            (s(&root2.join("node_modules")), SkipReason::Excluded, None),
            (s(&root2.join("target")), SkipReason::Excluded, None),
        ]
    );
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
    let paths: Vec<String> = plan.ops.iter().map(|o| s(&o.source)).collect();
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
    // Phase 1: changed intentionally: the same file was planned once per root that contains it -> overlapping roots are deduplicated, the file is planned once
    assert_eq!(sorted_paths(&plan), vec![s(&f)]);

    // Same root passed twice.
    let plan = env.plan(&[root.clone(), root]).await;
    // Phase 1: changed intentionally: a repeated root planned every file twice -> once
    assert_eq!(plan.ops.len(), 1);
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

    let plan = env.plan(std::slice::from_ref(&missing)).await;
    // Phase 1: changed intentionally: a missing root gave a silently empty plan -> still no ops and no error, but the root is reported as Unreadable
    assert!(plan.ops.is_empty());
    assert_eq!(
        skips(&plan),
        vec![(s(&missing), SkipReason::Unreadable, None)]
    );

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
    // Phase 1: changed intentionally: a file already in the destination folder was planned to move into that same folder again -> it is reported as AlreadyInPlace and only the new file is planned
    assert_eq!(sorted_paths(&plan), sorted_strings(&[&fresh]));
    assert_eq!(
        skips(&plan),
        vec![(
            s(&already),
            SkipReason::AlreadyInPlace,
            Some("sort".to_string())
        )]
    );
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
    env.add(&move_rule(
        2,
        "big",
        json!([{ "type": "sizeGt", "value": 10 }]),
    ))
    .await;

    let first = env.plan(std::slice::from_ref(&root)).await;
    let second = env.plan(std::slice::from_ref(&root)).await;
    // Phase 1: changed intentionally: 10 entries (pdf rule: 4 files x 2 actions; big rule: 2 files in sub) -> 5 ops (4 pdf moves to P, sub/y.txt moved by big) and one Conflict (sub/x.pdf also matches big)
    assert_eq!(first.ops.len(), 5);
    assert_eq!(
        skips(&first),
        vec![(
            s(&root.join("sub").join("x.pdf")),
            SkipReason::Conflict,
            Some("big".to_string())
        )]
    );
    assert_eq!(sorted_rows(&first), sorted_rows(&second));
    // Phase 1: changed intentionally: plan order followed the OS directory listing -> entries are sorted by file name, so the whole plan is identical
    assert_eq!(first, second);
    env.close().await;
}
