//! End to end on NTFS with the real `SystemPlatform` and valet-core's planner,
//! executor and journal (runs on windows-latest CI; needs `--features valet-core`).
//!
//! Covers: same-volume move + undo; an OFFLINE (cloud placeholder) file is
//! skipped by the planner and refused by the executor before hashing; a file
//! held without sharing fails with `Locked` and is untouched; a path over 300
//! characters moves (as source and as destination) and undo restores it; a
//! `trash` rule sends a file to the real Recycle Bin and undo restores it
//! byte-identical; an `emptyFolder` rule trashes an empty folder and undo
//! restores it; an occupied original path at undo needs confirmation and the
//! item stays in the bin. Trash tests restore everything they trash, so the
//! Recycle Bin is left as found.
#![cfg(windows)]

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::json;
use valet_core::executor::{
    BatchReport, ConfirmReason, EMPTY_DIR_MARK, FailCode, UndoReport, execute, undo_batch,
};
use valet_core::journal::{OpRecord, OpState, OpType};
use valet_core::planner::{OpKind, Plan, PlanContext, PlannedOp, SkipReason, plan};
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

/// A rule that sends what `conditions` match to the Recycle Bin.
fn trash_rule(n: u128, conditions: serde_json::Value) -> Rule {
    serde_json::from_value(json!({
        "id": uuid::Uuid::from_u128(n),
        "name": format!("trash-{n}"),
        "enabled": true,
        "conditions": conditions,
        "actions": [{ "type": "trash" }],
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
        self.undo_with(batch, false)
    }

    fn undo_with(&self, batch: &str, force: bool) -> UndoReport {
        self.rt
            .block_on(undo_batch(
                batch,
                &self.db.journal(),
                &SystemPlatform,
                force,
            ))
            .unwrap()
    }

    fn op(&self, id: i64) -> OpRecord {
        self.rt
            .block_on(self.db.journal().get_op(id))
            .unwrap()
            .unwrap()
    }

    fn ops(&self, batch: &str) -> Vec<OpRecord> {
        self.rt.block_on(self.db.journal().list_ops(batch)).unwrap()
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

/// The single `restore` step of `u`'s undo batch; checks it points at `orig`.
fn single_restore(env: &Env, u: &UndoReport, orig: &OpRecord) -> OpRecord {
    let ops = env.ops(u.undo_batch_id.as_ref().expect("undo batch"));
    assert_eq!(ops.len(), 1, "{ops:?}");
    let r = ops.into_iter().next().unwrap();
    assert_eq!(r.op_type, OpType::Restore);
    assert_eq!(r.state, OpState::Done);
    assert_eq!(r.undo_of, Some(orig.id));
    assert_eq!(r.source_path, orig.source_path);
    assert_eq!(r.hash_before, orig.hash_before);
    r
}

#[test]
fn trash_rule_sends_file_to_recycle_bin_and_undo_restores_it() {
    let env = Env::new();
    let root = env.root();
    let exe = root.join("valet_e2e_old_setup.exe");
    let body: Vec<u8> = (0..=255u8).cycle().take(70_000).collect();
    write(&exe, &body);
    write(&root.join("keep.txt"), b"stays");
    let before = tree(&root);

    let p = env.plan(&[trash_rule(1, json!([{ "type": "ext", "value": "exe" }]))]);
    assert_eq!(p.ops.len(), 1, "{p:?}");
    assert_eq!(p.ops[0].kind, OpKind::Trash);
    let report = env.execute(&p.ops);
    assert_all_done(&report);
    assert!(!exists(&exe), "still at source");
    assert!(exists(&root.join("keep.txt")));
    let orig = env.op(report.results[0].op_id);
    assert_eq!(orig.op_type, OpType::Trash);
    assert_eq!(orig.state, OpState::Done);
    assert_eq!(orig.source_path, exe);
    assert!(orig.dest_path.is_none());
    assert_eq!(orig.size, Some(body.len() as u64));
    assert!(orig.hash_before.is_some());

    let u = env.undo(&report.batch_id);
    assert_undo_clean(&u, 1);
    assert!(u.restored_modified.is_empty(), "{u:?}");
    assert_eq!(fs::read(&exe).unwrap(), body);
    assert_eq!(tree(&root), before);
    let r = single_restore(&env, &u, &orig);
    assert_eq!(r.hash_after, orig.hash_before, "restored bytes differ");
    assert_eq!(env.op(orig.id).state, OpState::RolledBack);
}

#[test]
fn empty_folder_rule_trashes_folder_and_undo_restores_it() {
    let env = Env::new();
    let root = env.root();
    let empty = root.join("valet_e2e_empty_folder");
    fs::create_dir_all(&empty).unwrap();
    write(&root.join("full").join("f.txt"), b"not empty");
    let before = tree(&root);

    let p = env.plan(&[trash_rule(
        1,
        json!([{ "type": "emptyFolder", "value": true }]),
    )]);
    assert_eq!(p.ops.len(), 1, "{p:?}");
    assert_eq!((p.ops[0].kind, &p.ops[0].source), (OpKind::Trash, &empty));
    let report = env.execute(&p.ops);
    assert_all_done(&report);
    assert!(!exists(&empty), "folder still there");
    assert!(exists(&root.join("full").join("f.txt")));
    let orig = env.op(report.results[0].op_id);
    assert_eq!(orig.op_type, OpType::Trash);
    assert_eq!(orig.state, OpState::Done);
    assert_eq!(orig.hash_before.as_deref(), Some(EMPTY_DIR_MARK));

    let u = env.undo(&report.batch_id);
    assert_undo_clean(&u, 1);
    assert!(u.restored_modified.is_empty(), "{u:?}");
    assert!(fs::symlink_metadata(&empty).unwrap().is_dir());
    assert_eq!(fs::read_dir(&empty).unwrap().count(), 0);
    assert_eq!(tree(&root), before);
    single_restore(&env, &u, &orig);
    assert_eq!(env.op(orig.id).state, OpState::RolledBack);
}

#[test]
fn undo_trash_onto_occupied_path_needs_confirmation_and_keeps_bin_item() {
    let env = Env::new();
    let root = env.root();
    let exe = root.join("valet_e2e_occupied.exe");
    write(&exe, b"trashed original");
    let p = env.plan(&[trash_rule(1, json!([{ "type": "ext", "value": "exe" }]))]);
    assert_eq!(p.ops.len(), 1, "{p:?}");
    let report = env.execute(&p.ops);
    assert_all_done(&report);
    let id = report.results[0].op_id;
    assert!(!exists(&exe));
    write(&exe, b"newcomer");

    // Without force: asks, journals nothing, touches nothing.
    let u = env.undo(&report.batch_id);
    assert!(u.undone.is_empty() && u.failed.is_empty(), "{u:?}");
    assert_eq!(u.needs_confirmation.len(), 1, "{u:?}");
    assert_eq!(u.needs_confirmation[0].op_id, id);
    assert_eq!(
        u.needs_confirmation[0].reasons,
        [ConfirmReason::SourceOccupied]
    );
    assert!(u.undo_batch_id.is_none(), "{u:?}");
    assert_eq!(fs::read(&exe).unwrap(), b"newcomer");

    // With force: fails with RestoreOccupied, the item stays in the bin.
    let u = env.undo_with(&report.batch_id, true);
    assert!(u.undone.is_empty(), "{u:?}");
    assert_eq!(u.failed.len(), 1, "{u:?}");
    assert_eq!(u.failed[0].error.code, FailCode::RestoreOccupied);
    assert_eq!(fs::read(&exe).unwrap(), b"newcomer");
    assert_eq!(env.op(id).state, OpState::Done);

    // Cleanup that also proves the item is still in the bin: move the
    // blocker aside and undo again. Not `trash_path` on the blocker:
    // `restore_from_trash` takes the most recent item from that path, which
    // would then be the blocker, not the original.
    let aside = root.join("valet_e2e_occupied (blocker).exe");
    fs::rename(&exe, &aside).unwrap();
    let u = env.undo(&report.batch_id);
    assert_undo_clean(&u, 1);
    assert_eq!(fs::read(&exe).unwrap(), b"trashed original");
    assert_eq!(fs::read(&aside).unwrap(), b"newcomer");
    assert_eq!(env.op(id).state, OpState::RolledBack);
}
