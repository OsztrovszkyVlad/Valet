//! `trash` operations: execution, recovery and undo (restore from the bin).

use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::tests::{Env, hash, planned, tree, write};
use super::*;
use crate::{journal::OpState, platform::testing::FakePlatform};

/// A trash entry as the planner makes it (folders have size 0).
fn trash_op(source: &Path) -> PlannedOp {
    let mut op = planned(source, Path::new(""), OpKind::Trash);
    if source.is_dir() {
        op.size = 0;
    }
    op
}

fn code(r: &OpResult) -> Option<FailCode> {
    r.error.as_ref().map(|e| e.code)
}

/// Names in the fake Recycle Bin, sorted.
fn bin(env: &Env) -> Vec<String> {
    let mut out: Vec<String> = match fs::read_dir(env.trash_dir()) {
        Ok(rd) => rd
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => vec![],
    };
    out.sort();
    out
}

fn set_mtime(path: &Path, t: SystemTime) {
    fs::File::open(path).unwrap().set_modified(t).unwrap();
}

#[tokio::test]
async fn trash_file_goes_to_the_bin_and_is_journaled() {
    let env = Env::new().await;
    let src = env.path("in/old.exe");
    write(&src, b"setup");
    let ok = env.path("in/ok.txt");
    write(&ok, b"ok");
    let p = env.fake();
    let ops = [
        trash_op(&src),
        planned(&ok, &env.path("out/ok.txt"), OpKind::Move),
    ];

    let rep = execute(&ops, &env.journal, &p).await.unwrap();
    assert_eq!((rep.done_count(), rep.failed_count()), (2, 0));
    let r = &rep.results[0];
    assert_eq!(r.kind, OpKind::Trash);
    assert_eq!(r.dest, PathBuf::new());
    assert!(!src.exists());
    assert_eq!(bin(&env), ["0-old.exe"]);
    assert_eq!(
        fs::read(env.trash_dir().join("0-old.exe")).unwrap(),
        b"setup"
    );
    let rec = env.op(r.op_id).await;
    assert_eq!(rec.op_type, OpType::Trash);
    assert_eq!(rec.state, OpState::Done);
    assert!(rec.dest_path.is_none());
    assert_eq!(rec.size, Some(5));
    assert_eq!(rec.hash_before, Some(hash(b"setup")));
    assert!(rec.hash_after.is_none());
}

#[tokio::test]
async fn trash_empty_folder_records_the_empty_mark() {
    let env = Env::new().await;
    let dir = env.path("in/empty");
    fs::create_dir_all(&dir).unwrap();
    let p = env.fake();
    let rep = execute(&[trash_op(&dir)], &env.journal, &p).await.unwrap();
    assert_eq!(rep.done_count(), 1, "{:?}", rep.results);
    assert!(!dir.exists());
    assert!(env.trash_dir().join("0-empty").is_dir());
    let rec = env.op(rep.results[0].op_id).await;
    assert_eq!(rec.hash_before.as_deref(), Some(EMPTY_DIR_MARK));
    assert_eq!(rec.state, OpState::Done);
}

#[tokio::test]
async fn trash_folder_that_is_no_longer_empty_fails_untouched() {
    let env = Env::new().await;
    let dir = env.path("in/was-empty");
    fs::create_dir_all(&dir).unwrap();
    let op = trash_op(&dir);
    write(&dir.join("new.txt"), b"new");
    // Same mtime as in the preview: the emptiness check catches it.
    set_mtime(&dir, op.mtime);
    let p = env.fake();
    let rep = execute(&[op], &env.journal, &p).await.unwrap();
    let r = &rep.results[0];
    assert_eq!(code(r), Some(FailCode::ChangedSincePreview));
    assert!(
        r.error
            .as_ref()
            .unwrap()
            .detail
            .as_ref()
            .unwrap()
            .contains("empty")
    );
    let rec = env.op(r.op_id).await;
    assert_eq!(rec.state, OpState::Failed);
    assert!(rec.hash_before.is_none(), "failed before in_progress");
    assert_eq!(fs::read(dir.join("new.txt")).unwrap(), b"new");
    assert!(bin(&env).is_empty());
}

/// Fills `dir` when the executor asks whether it can trash (after the
/// first emptiness check), so only the re-check right before trashing sees it.
struct FillOnCanTrash {
    inner: FakePlatform,
    dir: PathBuf,
}

impl Platform for FillOnCanTrash {
    fn trash(&self, p: &Path) -> io::Result<()> {
        self.inner.trash(p)
    }
    fn restore_from_trash(&self, p: &Path, after: SystemTime) -> io::Result<()> {
        self.inner.restore_from_trash(p, after)
    }
    fn can_trash(&self, p: &Path) -> bool {
        write(&self.dir.join("late.txt"), b"late");
        self.inner.can_trash(p)
    }
    fn is_cloud_placeholder(&self, p: &Path) -> bool {
        self.inner.is_cloud_placeholder(p)
    }
    fn is_locked(&self, p: &Path) -> bool {
        self.inner.is_locked(p)
    }
    fn is_hidden_or_system(&self, p: &Path) -> bool {
        self.inner.is_hidden_or_system(p)
    }
    fn is_reparse_point(&self, p: &Path) -> bool {
        self.inner.is_reparse_point(p)
    }
    fn same_volume(&self, a: &Path, b: &Path) -> io::Result<bool> {
        self.inner.same_volume(a, b)
    }
    fn rename_no_replace(&self, a: &Path, b: &Path) -> io::Result<()> {
        self.inner.rename_no_replace(a, b)
    }
    fn os_path(&self, p: &Path) -> PathBuf {
        self.inner.os_path(p)
    }
}

#[tokio::test]
async fn folder_filled_right_before_trash_fails_after_in_progress() {
    let env = Env::new().await;
    let dir = env.path("in/racy");
    fs::create_dir_all(&dir).unwrap();
    let p = FillOnCanTrash {
        inner: env.fake(),
        dir: dir.clone(),
    };
    let rep = execute(&[trash_op(&dir)], &env.journal, &p).await.unwrap();
    let r = &rep.results[0];
    assert_eq!(code(r), Some(FailCode::ChangedSincePreview));
    let rec = env.op(r.op_id).await;
    assert_eq!(rec.state, OpState::Failed);
    assert_eq!(rec.hash_before.as_deref(), Some(EMPTY_DIR_MARK));
    assert_eq!(fs::read(dir.join("late.txt")).unwrap(), b"late");
    assert!(bin(&env).is_empty());
}

#[tokio::test]
async fn trash_prechecks_fail_without_touching() {
    let env = Env::new().await;
    let gone = env.path("in/gone.exe");
    write(&gone, b"g");
    let gone_op = trash_op(&gone);
    fs::rename(&gone, env.path("elsewhere.exe")).unwrap();

    let cloud = env.path("in/cloud.exe");
    write(&cloud, b"c");
    let changed = env.path("in/changed.exe");
    write(&changed, b"before");
    let changed_op = trash_op(&changed);
    write(&changed, b"after, longer");
    let locked = env.path("in/locked.exe");
    write(&locked, b"l");

    let mut p = env.fake();
    p.placeholders.insert(cloud.clone());
    p.locked.insert(locked.clone());
    let ops = [gone_op, trash_op(&cloud), changed_op, trash_op(&locked)];
    let rep = execute(&ops, &env.journal, &p).await.unwrap();
    let codes: Vec<_> = rep.results.iter().map(code).collect();
    assert_eq!(
        codes,
        [
            Some(FailCode::SourceMissing),
            Some(FailCode::CloudPlaceholder),
            Some(FailCode::ChangedSincePreview),
            Some(FailCode::Locked),
        ]
    );
    for r in &rep.results {
        let rec = env.op(r.op_id).await;
        assert_eq!(rec.state, OpState::Failed);
        assert!(rec.hash_before.is_none(), "never hashed");
    }
    assert!(cloud.exists() && changed.exists() && locked.exists());
    assert!(bin(&env).is_empty());
}

#[tokio::test]
async fn trash_without_recycle_bin_fails_untouched() {
    let env = Env::new().await;
    let src = env.path("in/old.msi");
    write(&src, b"msi");
    let dir = env.path("in/empty");
    fs::create_dir_all(&dir).unwrap();
    let mut p = env.fake();
    p.can_trash = false;
    let rep = execute(&[trash_op(&src), trash_op(&dir)], &env.journal, &p)
        .await
        .unwrap();
    for r in &rep.results {
        assert_eq!(code(r), Some(FailCode::NoRecycleBin));
    }
    assert!(src.exists() && dir.is_dir());
    assert!(bin(&env).is_empty());
}

#[tokio::test]
async fn refused_trash_fails_and_keeps_the_file() {
    let env = Env::new().await;
    let src = env.path("in/old.exe");
    write(&src, b"x");
    let mut p = env.fake();
    p.trash_fails.insert(src.clone());
    let rep = execute(&[trash_op(&src)], &env.journal, &p).await.unwrap();
    assert_eq!(code(&rep.results[0]), Some(FailCode::TrashFailed));
    let rec = env.op(rep.results[0].op_id).await;
    assert_eq!(rec.state, OpState::Failed);
    assert_eq!(rec.hash_before, Some(hash(b"x")), "was in progress");
    assert_eq!(fs::read(&src).unwrap(), b"x");
}

// ---------------------------------------------------------------- recovery

#[tokio::test]
async fn interrupted_trash_is_decided_by_the_disk() {
    let env = Env::new().await;
    let p = env.fake();
    let b = env.journal.create_batch(BatchKind::Apply).await.unwrap();
    let in_bin = env.path("in/in-bin.exe");
    write(&in_bin, b"bin");
    let still = env.path("in/still.exe");
    write(&still, b"still");
    let mut ids = vec![];
    for (seq, path) in [(0, &in_bin), (1, &still)] {
        let id = env
            .journal
            .insert_planned(
                &b,
                seq,
                &NewOp {
                    op_type: OpType::Trash,
                    source: path.clone(),
                    dest: None,
                    size: Some(3),
                    undo_of: None,
                },
            )
            .await
            .unwrap();
        env.journal
            .start(id, Some(&hash(&fs::read(path).unwrap())))
            .await
            .unwrap();
        ids.push(id);
    }
    // The crash came after the first file reached the bin.
    p.trash(&in_bin).unwrap();
    let rep = recover(&env.journal, &p).await.unwrap();
    let state = |id| {
        rep.decisions
            .iter()
            .find(|d| d.op_id == id)
            .unwrap()
            .new_state
    };
    assert_eq!(state(ids[0]), OpState::Done);
    assert_eq!(state(ids[1]), OpState::Failed);
    assert!(
        env.op(ids[1])
            .await
            .error
            .unwrap()
            .starts_with("Interrupted")
    );
    assert_eq!(fs::read(&still).unwrap(), b"still");
    assert!(
        env.journal
            .get_batch(&b)
            .await
            .unwrap()
            .unwrap()
            .finished_at
            .is_some()
    );

    // Recovered batch: the trashed file can be restored by undo.
    let u = undo_batch(&b, &env.journal, &p, false).await.unwrap();
    assert_eq!(u.undone, [ids[0]]);
    assert_eq!(fs::read(&in_bin).unwrap(), b"bin");
}

// ---------------------------------------------------------------- undo

#[tokio::test]
async fn undo_restores_trashed_files_and_folders_with_moves() {
    let env = Env::new().await;
    let root = env.path("root");
    let exe = root.join("Downloads/old.exe");
    write(&exe, b"installer bytes");
    let pdf = root.join("Downloads/a.pdf");
    write(&pdf, b"pdf");
    let empty = root.join("Downloads/Gol");
    fs::create_dir_all(&empty).unwrap();
    let before = tree(&root);
    let p = env.fake();
    let ops = [
        trash_op(&exe),
        planned(&pdf, &root.join("Docs/a.pdf"), OpKind::Move),
        trash_op(&empty),
    ];
    let rep = execute(&ops, &env.journal, &p).await.unwrap();
    assert_eq!(rep.failed_count(), 0, "{:?}", rep.results);
    assert!(!exe.exists() && !empty.exists());

    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert_eq!(u.undone.len(), 3, "{u:?}");
    assert!(u.failed.is_empty() && u.needs_confirmation.is_empty());
    assert!(u.restored_modified.is_empty());
    assert_eq!(tree(&root), before);
    assert!(empty.is_dir());
    assert!(bin(&env).is_empty());
    for r in &rep.results {
        assert_eq!(env.op(r.op_id).await.state, OpState::RolledBack);
    }
    let undo_ops = env
        .journal
        .list_ops(u.undo_batch_id.as_ref().unwrap())
        .await
        .unwrap();
    let restores: Vec<_> = undo_ops
        .iter()
        .filter(|o| o.op_type == OpType::Restore)
        .collect();
    assert_eq!(restores.len(), 2);
    for o in &restores {
        assert_eq!(o.state, OpState::Done);
        assert_eq!(o.dest_path.as_ref(), Some(&o.source_path));
    }
    let exe_restore = restores.iter().find(|o| o.source_path == exe).unwrap();
    assert_eq!(exe_restore.hash_before, Some(hash(b"installer bytes")));
    assert_eq!(exe_restore.hash_after, Some(hash(b"installer bytes")));
}

#[tokio::test]
async fn undo_trash_onto_occupied_path_asks_then_fails_with_restore_occupied() {
    let env = Env::new().await;
    let src = env.path("in/old.exe");
    write(&src, b"old");
    let p = env.fake();
    let rep = execute(&[trash_op(&src)], &env.journal, &p).await.unwrap();
    let id = rep.results[0].op_id;
    write(&src, b"newcomer");

    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert_eq!(u.needs_confirmation.len(), 1);
    assert_eq!(
        u.needs_confirmation[0].reasons,
        [ConfirmReason::SourceOccupied]
    );
    assert!(u.undo_batch_id.is_none(), "nothing journaled");

    let u = undo_batch(&rep.batch_id, &env.journal, &p, true)
        .await
        .unwrap();
    assert_eq!(u.failed.len(), 1);
    assert_eq!(u.failed[0].error.code, FailCode::RestoreOccupied);
    assert_eq!(fs::read(&src).unwrap(), b"newcomer");
    assert_eq!(bin(&env), ["0-old.exe"], "item stays in the bin");
    assert_eq!(env.op(id).await.state, OpState::Done);

    // Once the path is free, undo restores it.
    fs::rename(&src, env.path("in/newcomer.exe")).unwrap();
    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert_eq!(u.undone, [id]);
    assert_eq!(fs::read(&src).unwrap(), b"old");
}

#[tokio::test]
async fn undo_trash_with_changed_content_restores_and_reports_it() {
    let env = Env::new().await;
    let src = env.path("in/old.exe");
    write(&src, b"old");
    let p = env.fake();
    let rep = execute(&[trash_op(&src)], &env.journal, &p).await.unwrap();
    let id = rep.results[0].op_id;
    // The item changed while in the bin.
    fs::write(env.trash_dir().join("0-old.exe"), b"changed").unwrap();

    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert_eq!(u.undone, [id]);
    assert_eq!(u.restored_modified.len(), 1);
    assert_eq!(u.restored_modified[0].path, src);
    assert_eq!(fs::read(&src).unwrap(), b"changed");
    assert_eq!(env.op(id).await.state, OpState::RolledBack);
}

#[tokio::test]
async fn undo_trash_missing_from_the_bin_fails_and_keeps_the_op() {
    let env = Env::new().await;
    let src = env.path("in/old.exe");
    write(&src, b"old");
    let p = env.fake();
    let rep = execute(&[trash_op(&src)], &env.journal, &p).await.unwrap();
    let id = rep.results[0].op_id;
    // Emptied from the bin by hand (moved out here; nothing is deleted).
    fs::rename(env.trash_dir().join("0-old.exe"), env.path("rescued.exe")).unwrap();

    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert_eq!(u.failed.len(), 1);
    assert_eq!(u.failed[0].error.code, FailCode::SourceMissing);
    assert_eq!(env.op(id).await.state, OpState::Done);
    let undo_ops = env
        .journal
        .list_ops(u.undo_batch_id.as_ref().unwrap())
        .await
        .unwrap();
    assert_eq!(undo_ops.len(), 1);
    assert_eq!(undo_ops[0].op_type, OpType::Restore);
    assert_eq!(undo_ops[0].state, OpState::Failed);
    assert!(!src.exists());
}
