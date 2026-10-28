//! End to end on NTFS with the real `SystemPlatform` and valet-core's planner,
//! executor and journal (runs on windows-latest CI; needs `--features valet-core`).
//!
//! Covers: same-volume move + undo; an OFFLINE (cloud placeholder) file is
//! skipped by the planner and refused by the executor before hashing; a file
//! held without sharing fails with `Locked` and is untouched; a path over 300
//! characters moves (as source and as destination) and undo restores it.
#![cfg(windows)]

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::json;
use valet_core::executor::{BatchReport, FailCode, UndoReport, execute, undo_batch};
use valet_core::journal::OpState;
use valet_core::planner::{Plan, PlanContext, PlannedOp, SkipReason, plan};
use valet_core::platform::Platform;
use valet_core::rules::Rule;
use valet_core::storage::Db;
use valet_platform::{SystemPlatform, to_long_path};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_OFFLINE, SetFileAttributesW,
};

type Tree = BTreeMap<PathBuf, Vec<u8>>;

fn wide(p: &Path) -> Vec<u16> {
    OsStr::new(p)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn set_attrs(path: &Path, attrs: u32) {
    let w = wide(&to_long_path(path));
    // SAFETY: `w` is a NUL-terminated UTF-16 path.
    let ok = unsafe { SetFileAttributesW(w.as_ptr(), attrs) };
    assert!(
        ok != 0,
        "SetFileAttributesW: {}",
        std::io::Error::last_os_error()
    );
}

fn write(path: &Path, body: &[u8]) {
    fs::create_dir_all(to_long_path(path.parent().unwrap())).unwrap();
    fs::write(to_long_path(path), body).unwrap();
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(to_long_path(path)).is_ok()
}

/// A directory under `base` whose path is longer than 300 characters.
fn deep_dir(base: &Path) -> PathBuf {
    let mut p = base.to_path_buf();
    let seg = "valet_e2e_long_path_segment_0123456789_abcdefghijklmnopqrstuvwxyz";
    while p.as_os_str().len() < 300 {
        p.push(seg);
    }
    fs::create_dir_all(to_long_path(&p)).unwrap();
    p
}

/// Relative path -> bytes of every file under `root` (long-path safe).
fn tree(root: &Path) -> Tree {
    fn walk(dir: &Path, rel: &Path, out: &mut Tree) {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let ft = e.file_type().unwrap();
            let r = rel.join(e.file_name());
            if ft.is_dir() {
                walk(&e.path(), &r, out);
            } else if ft.is_file() {
                out.insert(r, fs::read(e.path()).unwrap());
            }
        }
    }
    let mut out = Tree::new();
    walk(&to_long_path(root), Path::new(""), &mut out);
    out
}

fn rule(n: u128, ext: &str, path: &str) -> Rule {
    serde_json::from_value(json!({
        "id": uuid::Uuid::from_u128(n),
        "name": format!("rule-{ext}"),
        "enabled": true,
        "conditions": [{ "type": "ext", "value": ext }],
        "actions": [{ "type": "moveTo", "params": { "path": path } }],
    }))
    .unwrap()
}

fn ctx() -> PlanContext {
    PlanContext {
        // Fixture files are seconds old; move the clock past the
        // being-written window instead of backdating every file.
        now: SystemTime::now() + Duration::from_secs(3600),
        home_dir: None,
        env: Box::new(|_| None),
        folders: Default::default(),
    }
}

struct Env {
    tmp: tempfile::TempDir,
    rt: tokio::runtime::Runtime,
    db: Db,
}

impl Env {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let db = rt
            .block_on(Db::connect(&tmp.path().join("state").join("valet.sqlite3")))
            .unwrap();
        fs::create_dir_all(tmp.path().join("root")).unwrap();
        Self { tmp, rt, db }
    }

    fn root(&self) -> PathBuf {
        self.tmp.path().join("root")
    }

    fn plan(&self, rules: &[Rule]) -> Plan {
        plan(&[self.root()], rules, &SystemPlatform, &ctx())
    }

    fn execute(&self, ops: &[PlannedOp]) -> BatchReport {
        self.rt
            .block_on(execute(ops, &self.db.journal(), &SystemPlatform))
            .unwrap()
    }

    fn undo(&self, batch: &str) -> UndoReport {
        self.rt
            .block_on(undo_batch(
                batch,
                &self.db.journal(),
                &SystemPlatform,
                false,
            ))
            .unwrap()
    }
}

fn assert_all_done(r: &BatchReport) {
    let failed: Vec<_> = r.results.iter().filter(|x| x.error.is_some()).collect();
    assert!(failed.is_empty(), "failed ops: {failed:?}");
}

fn assert_undo_clean(u: &UndoReport, n: usize) {
    assert_eq!(u.undone.len(), n, "{u:?}");
    assert!(
        u.failed.is_empty() && u.needs_confirmation.is_empty(),
        "{u:?}"
    );
    assert!(u.restored_as.is_empty(), "{u:?}");
}

#[test]
fn same_volume_move_then_undo_restores_tree() {
    let env = Env::new();
    let root = env.root();
    write(&root.join("a.pdf"), b"pdf a");
    write(&root.join("sub").join("b.pdf"), b"pdf b");
    write(&root.join("c.jpg"), b"jpg c");
    write(&root.join("keep.txt"), b"stays");
    // Name taken at the destination: must get "a (2).pdf", never overwrite.
    write(
        &root.join("Sorted").join("pdf").join("a.pdf"),
        b"pre-existing",
    );
    let before = tree(&root);

    let p = env.plan(&[
        rule(1, "pdf", "Sorted/{ext}"),
        rule(2, "jpg", "Sorted/{ext}"),
    ]);
    assert_eq!(p.ops.len(), 3, "{p:?}");
    let report = env.execute(&p.ops);
    assert_all_done(&report);
    for r in &report.results {
        assert!(!exists(&r.source), "{} still at source", r.source.display());
        assert!(exists(&r.dest), "{} missing", r.dest.display());
    }
    assert!(exists(&root.join("Sorted").join("pdf").join("a (2).pdf")));
    assert_eq!(
        fs::read(root.join("Sorted").join("pdf").join("a.pdf")).unwrap(),
        b"pre-existing"
    );
    assert!(exists(&root.join("keep.txt")));

    let u = env.undo(&report.batch_id);
    assert_undo_clean(&u, 3);
    assert_eq!(tree(&root), before);
}

#[test]
fn offline_file_is_skipped_and_never_hashed() {
    let env = Env::new();
    let root = env.root();
    let cloud = root.join("cloud.pdf");
    write(&cloud, b"placeholder bytes");
    write(&root.join("local.pdf"), b"local");
    let meta = fs::metadata(&cloud).unwrap();
    set_attrs(&cloud, FILE_ATTRIBUTE_OFFLINE);
    assert!(SystemPlatform.is_cloud_placeholder(&cloud));

    // Planner: reported as CloudPlaceholder, no op.
    let p = env.plan(&[rule(1, "pdf", "Sorted")]);
    assert!(p.ops.iter().all(|o| o.source != cloud), "{p:?}");
    assert!(
        p.skipped
            .iter()
            .any(|s| s.path == cloud && s.reason == SkipReason::CloudPlaceholder),
        "{:?}",
        p.skipped
    );
    assert_eq!(p.ops.len(), 1);

    // Executor: a stale plan entry for the placeholder is refused before
    // hashing (no hash_before in the journal) and the file stays.
    let stale = PlannedOp {
        source: cloud.clone(),
        dest: root.join("Sorted").join("cloud.pdf"),
        kind: valet_core::planner::OpKind::Move,
        rule_id: uuid::Uuid::from_u128(1),
        rule_name: "rule-pdf".into(),
        size: meta.len(),
        mtime: meta.modified().unwrap(),
    };
    let report = env.execute(&[stale]);
    let r = &report.results[0];
    assert_eq!(
        r.error.as_ref().map(|e| e.code),
        Some(FailCode::CloudPlaceholder)
    );
    let op = env
        .rt
        .block_on(env.db.journal().get_op(r.op_id))
        .unwrap()
        .unwrap();
    assert_eq!(op.state, OpState::Failed);
    assert_eq!(op.hash_before, None, "placeholder was hashed");
    assert!(exists(&cloud));
    assert!(!exists(&root.join("Sorted").join("cloud.pdf")));

    set_attrs(&cloud, FILE_ATTRIBUTE_NORMAL);
    assert_eq!(fs::read(&cloud).unwrap(), b"placeholder bytes");
}

#[test]
fn locked_file_fails_with_locked_and_is_untouched() {
    let env = Env::new();
    let root = env.root();
    let f = root.join("busy.pdf");
    write(&f, b"in use");
    let p = env.plan(&[rule(1, "pdf", "Sorted")]);
    assert_eq!(p.ops.len(), 1);

    let held = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&f)
        .unwrap();
    let report = env.execute(&p.ops);
    drop(held);

    let r = &report.results[0];
    assert_eq!(
        r.error.as_ref().map(|e| e.code),
        Some(FailCode::Locked),
        "{r:?}"
    );
    assert_eq!(fs::read(&f).unwrap(), b"in use");
    assert!(!exists(&root.join("Sorted").join("busy.pdf")));
    let op = env
        .rt
        .block_on(env.db.journal().get_op(r.op_id))
        .unwrap()
        .unwrap();
    assert_eq!(op.state, OpState::Failed);
}

#[test]
fn long_paths_move_and_undo() {
    let env = Env::new();
    let root = env.root();
    // Source over 300 characters, destination short.
    let deep_src = deep_dir(&root.join("deep"));
    let long_file = deep_src.join("long_source.pdf");
    write(&long_file, b"long source");
    assert!(long_file.as_os_str().len() > 300);
    // Source short, destination folder over 300 characters (outside the root).
    let deep_dest = deep_dir(&env.tmp.path().join("out"));
    write(&root.join("to_deep.jpg"), b"to deep");
    let before = tree(&root);

    let p = env.plan(&[
        rule(1, "pdf", "Sorted"),
        rule(2, "jpg", &deep_dest.to_string_lossy()),
    ]);
    assert_eq!(p.ops.len(), 2, "{p:?}");
    let report = env.execute(&p.ops);
    assert_all_done(&report);
    assert!(!exists(&long_file));
    assert_eq!(
        fs::read(to_long_path(&root.join("Sorted").join("long_source.pdf"))).unwrap(),
        b"long source"
    );
    let deep_target = deep_dest.join("to_deep.jpg");
    assert!(deep_target.as_os_str().len() > 300);
    assert_eq!(fs::read(to_long_path(&deep_target)).unwrap(), b"to deep");

    let u = env.undo(&report.batch_id);
    assert_undo_clean(&u, 2);
    assert_eq!(tree(&root), before);
    assert!(!exists(&deep_target));
}
