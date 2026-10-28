//! Integration on a synthetic tree (PLAN.md 4.6 "integrare", rust-testing
//! suite 5): plan -> execute (FakePlatform, real journal) -> undo.
//!
//! Checks: expected number of moves; nothing under `.git`, `node_modules`, a
//! `.valetignore` folder, a dot folder, or any hidden file is moved;
//! pre-existing destination files are byte-identical afterwards (no
//! overwrite); every moved file has its original blake3 at its destination;
//! undo restores the byte-identical tree. Prints plan, execute and undo times.
//!
//! `integration_10k` takes about 80 s in a debug build on the dev VM
//! (execute 45 s, undo 30 s); `integration_1k` is the quick variant. CI also
//! runs both in release.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde_json::json;
use uuid::Uuid;
use valet_core::executor::{execute, undo_batch};
use valet_core::planner::{PlanContext, SkipReason, plan};
use valet_core::platform::testing::FakePlatform;
use valet_core::rules::Rule;
use valet_core::storage::Db;

type Tree = BTreeMap<PathBuf, String>;

/// (condition type, value, destination template), in rule order.
const RULES: &[(&str, &str, &str)] = &[
    ("nameMatches", "(?i)^invoice", "Invoices"),
    ("ext", "pdf", "Sorted/{ext}"),
    ("ext", "jpg", "Sorted/{ext}"),
    ("ext", "png", "Images"),
    ("ext", "docx", "Sorted/{ext}"),
    ("ext", "zip", "Archives"),
];
/// Extensions of eligible files; "" = no extension. txt, mp4 and no-extension
/// files match no rule and stay.
const EXTS: &[&str] = &["pdf", "jpg", "png", "docx", "txt", "zip", "mp4", "PDF", ""];

fn rules() -> Vec<Rule> {
    RULES
        .iter()
        .enumerate()
        .map(|(i, (t, v, path))| {
            serde_json::from_value(json!({
                "id": Uuid::from_u128(i as u128 + 1),
                "name": format!("rule{i}"),
                "enabled": true,
                "conditions": [{ "type": t, "value": v }],
                "actions": [{ "type": "moveTo", "params": { "path": path } }],
            }))
            .unwrap()
        })
        .collect()
}

/// Index of the first rule matching `name`, and its destination folder
/// relative to the scan root (independent of the planner's code).
fn expected_rule(name: &str) -> Option<(usize, String)> {
    let ext = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase());
    RULES.iter().enumerate().find_map(|(i, (t, v, tpl))| {
        let hit = match *t {
            "nameMatches" => name.to_lowercase().starts_with("invoice"),
            "ext" => ext.as_deref() == Some(*v),
            _ => unreachable!(),
        };
        hit.then(|| (i, tpl.replace("{ext}", ext.as_deref().unwrap_or(""))))
    })
}

fn write_old(path: &Path, body: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut f = File::create(path).unwrap();
    f.write_all(body).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
}

fn tree(dir: &Path, skip: &[&Path]) -> Tree {
    let mut out = Tree::new();
    for e in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| !skip.contains(&e.path()))
    {
        let e = e.unwrap();
        if e.file_type().is_file() {
            out.insert(
                e.path().strip_prefix(dir).unwrap().to_path_buf(),
                blake3::hash(&fs::read(e.path()).unwrap())
                    .to_hex()
                    .to_string(),
            );
        }
    }
    out
}

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    db: PathBuf,
    trash: PathBuf,
    /// Relative paths that must never be moved (excluded or hidden).
    protected: Vec<PathBuf>,
    /// Pre-existing files at destination names.
    pre_existing: Vec<PathBuf>,
    /// Paths with the simulated hidden attribute.
    hidden_attr: HashSet<PathBuf>,
    /// Expected moves: relative source -> relative destination folder.
    expected: HashMap<PathBuf, String>,
    total_files: usize,
}

/// Writes files with unique content and old mtimes, counting them.
struct Writer {
    root: PathBuf,
    count: usize,
}

impl Writer {
    fn put(&mut self, rel: &str) -> PathBuf {
        write_old(
            &self.root.join(rel),
            format!("{}:{rel}", self.count).as_bytes(),
        );
        self.count += 1;
        PathBuf::from(rel)
    }
}

/// Name of eligible file number `i` (see `fixture`).
fn eligible_name(i: usize) -> String {
    let ext = EXTS[i % EXTS.len()];
    if i.is_multiple_of(10) {
        format!("Invoice_{i:05}.pdf")
    } else if i % 97 == 3 {
        // Same name in many folders: collisions among sources.
        "dup.pdf".to_owned()
    } else if ext.is_empty() {
        format!("file_{i:05}")
    } else {
        format!("file_{i:05}.{ext}")
    }
}

/// Builds `n` files under `<tmp>/Downloads`: about 4% excluded or hidden,
/// 22 pre-existing files at destination names, the rest eligible.
fn fixture(n: usize) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Downloads");
    let state = tmp.path().join("state");
    fs::create_dir_all(&state).unwrap();
    let mut w = Writer {
        root: root.clone(),
        count: 0,
    };
    let mut protected = Vec::new();
    let mut hidden_attr = HashSet::new();

    let k = (n / 100).max(5);
    for i in 0..k {
        protected.push(w.put(&format!(".git/objects/{:02}/obj_{i}.pdf", i % 7)));
        protected.push(w.put(&format!("node_modules/pkg_{}/file_{i}.pdf", i % 5)));
        protected.push(w.put(&format!("project/sub/doc_{i}.pdf")));
        protected.push(w.put(&format!("d01/node_modules/x/m_{i}.jpg")));
    }
    protected.push(w.put("project/.valetignore"));
    for i in 0..5 {
        protected.push(w.put(&format!(".hidden_{i}.pdf")));
        protected.push(w.put(&format!("d02/.hidden_{i}.jpg")));
        protected.push(w.put(&format!(".cache/c_{i}.png")));
        let rel = w.put(&format!("d03/attr_hidden_{i}.pdf"));
        hidden_attr.insert(root.join(&rel));
        protected.push(rel);
    }

    // Pre-existing files in destination folders, named like eligible pdfs.
    let mut pre_existing = Vec::new();
    let pdf_names = (1..)
        .map(|m| 9 * m)
        .filter(|i| i % 10 != 0 && i % 97 != 3)
        .take(20)
        .map(eligible_name);
    for name in pdf_names.chain(["dup.pdf".to_owned()]) {
        pre_existing.push(w.put(&format!("Sorted/pdf/{name}")));
    }
    pre_existing.push(w.put("Invoices/Invoice_00010.pdf"));

    // Eligible files fill up to `n`.
    let mut expected = HashMap::new();
    let mut i = 0usize;
    while w.count < n {
        let name = eligible_name(i);
        let rel = match i % 3 {
            0 => format!("d{:02}/{name}", i % 50),
            1 => format!("d{:02}/s{:02}/{name}", i % 50, i % 7),
            _ => name.clone(),
        };
        i += 1;
        if root.join(&rel).exists() {
            continue;
        }
        let rel = w.put(&rel);
        if let Some((_, folder)) = expected_rule(&name) {
            expected.insert(rel, folder);
        }
    }
    Fixture {
        _tmp: tmp,
        root,
        db: state.join("valet.sqlite3"),
        trash: state.join("trash"),
        protected,
        pre_existing,
        hidden_attr,
        expected,
        total_files: w.count,
    }
}

fn ctx() -> PlanContext {
    PlanContext {
        now: SystemTime::now(),
        home_dir: None,
        env: Box::new(|_| None),
        // Phase 2: changed intentionally: PlanContext gained folders
        folders: Default::default(),
    }
}

fn run(n: usize) {
    let f = fixture(n);
    let mut platform = FakePlatform::with_trash_dir(&f.trash);
    platform.hidden = f.hidden_attr.clone();
    let before = tree(&f.root, &[]);
    assert_eq!(before.len(), f.total_files);

    let t = Instant::now();
    let p = plan(std::slice::from_ref(&f.root), &rules(), &platform, &ctx());
    let plan_time = t.elapsed();

    // Expected moves.
    assert_eq!(
        p.ops.len(),
        f.expected.len(),
        "ops {} vs expected {}",
        p.ops.len(),
        f.expected.len()
    );
    let mut dests = HashSet::new();
    for op in &p.ops {
        let rel = op.source.strip_prefix(&f.root).unwrap().to_path_buf();
        let folder = f
            .expected
            .get(&rel)
            .unwrap_or_else(|| panic!("unexpected op for {}", rel.display()));
        assert_eq!(
            op.dest.parent().unwrap(),
            f.root.join(folder),
            "{}",
            rel.display()
        );
        assert!(dests.insert(op.dest.to_string_lossy().to_lowercase()));
        assert!(!op.dest.exists(), "dest {} exists", op.dest.display());
    }
    // Pre-existing destination files are already in place, not moved.
    for rel in &f.pre_existing {
        assert!(p.ops.iter().all(|o| o.source != f.root.join(rel)));
        assert!(
            p.skipped
                .iter()
                .any(|s| s.path == f.root.join(rel) && s.reason == SkipReason::AlreadyInPlace),
            "{} not reported AlreadyInPlace",
            rel.display()
        );
    }
    let protected: HashSet<PathBuf> = f.protected.iter().map(|r| f.root.join(r)).collect();
    assert!(p.ops.iter().all(|o| !protected.contains(&o.source)));
    for dir in [".git", "node_modules", "project", ".cache"] {
        let d = f.root.join(dir);
        assert!(
            p.skipped.iter().any(|s| s.path == d),
            "{dir} not reported as skipped"
        );
    }
    let collided = p
        .ops
        .iter()
        .filter(|o| o.dest.file_name() != o.source.file_name())
        .count();
    assert!(collided > 0, "fixture should produce renamed destinations");

    // Execute.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (report, exec_time, undo, undo_time) = rt.block_on(async {
        let db = Db::connect(&f.db).await.unwrap();
        let j = db.journal();
        let t = Instant::now();
        let report = execute(&p.ops, &j, &platform).await.unwrap();
        let exec_time = t.elapsed();

        let after = tree(&f.root, &[]);
        check_after_execute(&f, &before, &after, &report);

        let t = Instant::now();
        let undo = undo_batch(&report.batch_id, &j, &platform, false)
            .await
            .unwrap();
        let undo_time = t.elapsed();
        db.pool.close().await;
        (report, exec_time, undo, undo_time)
    });
    assert_eq!(report.failed_count(), 0);
    assert_eq!(
        undo.undone.len(),
        p.ops.len(),
        "undo: {:?}",
        undo.failed.first()
    );
    assert!(undo.failed.is_empty() && undo.needs_confirmation.is_empty());
    assert!(undo.restored_as.is_empty());

    let restored = tree(&f.root, &[]);
    assert_eq!(restored.len(), before.len());
    assert!(
        restored == before,
        "undo did not restore a byte-identical tree"
    );
    // Nothing went to the Recycle Bin on the same-volume path.
    assert!(!f.trash.exists() || fs::read_dir(&f.trash).unwrap().next().is_none());

    let secs = |d: Duration| d.as_secs_f64();
    println!(
        "integration {n}: {} files on disk, {} ops ({collided} renamed on collision), {} skipped; \
         plan {:.3} s; execute {:.2} s ({:.0} files/s); undo {:.2} s ({:.0} files/s)",
        f.total_files,
        p.ops.len(),
        p.skipped.len(),
        secs(plan_time),
        secs(exec_time),
        p.ops.len() as f64 / secs(exec_time),
        secs(undo_time),
        p.ops.len() as f64 / secs(undo_time),
    );
}

fn check_after_execute(
    f: &Fixture,
    before: &Tree,
    after: &Tree,
    report: &valet_core::executor::BatchReport,
) {
    assert_eq!(
        report.done_count(),
        report.results.len(),
        "{:?}",
        report.results.iter().find(|r| r.error.is_some())
    );
    let mut moved_from = HashSet::new();
    for r in &report.results {
        let src = r.source.strip_prefix(&f.root).unwrap().to_path_buf();
        let dst = r.dest.strip_prefix(&f.root).unwrap().to_path_buf();
        assert!(
            !after.contains_key(&src),
            "{} still at source",
            src.display()
        );
        assert_eq!(
            after.get(&dst),
            before.get(&src),
            "{} at {}",
            src.display(),
            dst.display()
        );
        moved_from.insert(src);
    }
    // Everything not moved is unchanged (covers protected and pre-existing files).
    for (rel, h) in before {
        if !moved_from.contains(rel) {
            assert_eq!(after.get(rel), Some(h), "{} changed", rel.display());
        }
    }
    assert_eq!(after.len(), before.len(), "file count changed");
}

#[test]
fn integration_1k() {
    run(1_000);
}

#[test]
fn integration_10k() {
    run(10_000);
}
