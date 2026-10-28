use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use uuid::Uuid;

use super::*;
use crate::{
    journal::{OpRecord, OpState},
    platform::testing::FakePlatform,
    storage::Db,
};

pub(super) struct Env {
    pub tmp: tempfile::TempDir,
    pub _db: Db,
    pub journal: Journal,
}

impl Env {
    pub async fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("db").join("valet.sqlite3"))
            .await
            .unwrap();
        let journal = db.journal();
        Self {
            tmp,
            _db: db,
            journal,
        }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.tmp.path().join(rel)
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.path("trash")
    }

    pub fn fake(&self) -> FakePlatform {
        FakePlatform::with_trash_dir(self.trash_dir())
    }

    pub async fn op(&self, id: i64) -> OpRecord {
        self.journal.get_op(id).await.unwrap().unwrap()
    }
}

pub(super) fn write(path: &Path, body: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

pub(super) fn hash(body: &[u8]) -> String {
    blake3::hash(body).to_hex().to_string()
}

/// A plan entry for an existing file, as the planner would produce it.
pub(super) fn planned(source: &Path, dest: &Path, kind: OpKind) -> PlannedOp {
    let meta = fs::metadata(source).unwrap();
    PlannedOp {
        source: source.to_path_buf(),
        dest: dest.to_path_buf(),
        kind,
        rule_id: Uuid::nil(),
        rule_name: "r".into(),
        size: meta.len(),
        mtime: meta.modified().unwrap(),
    }
}

/// Contents of the fake Recycle Bin, sorted.
pub(super) fn trashed(env: &Env) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = match fs::read_dir(env.trash_dir()) {
        Ok(rd) => rd.map(|e| fs::read(e.unwrap().path()).unwrap()).collect(),
        Err(_) => vec![],
    };
    out.sort();
    out
}

/// Sorted (relative path, blake3) of every file under `dir`.
pub(super) fn tree(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = walkdir::WalkDir::new(dir)
        .into_iter()
        .map(|e| e.unwrap())
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path().strip_prefix(dir).unwrap().to_path_buf(),
                hash(&fs::read(e.path()).unwrap()),
            )
        })
        .collect();
    out.sort();
    out
}

fn temp_files(dir: &Path) -> Vec<PathBuf> {
    match fs::read_dir(dir) {
        Ok(rd) => rd
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".valet-tmp-")
            })
            .collect(),
        Err(_) => vec![],
    }
}

fn code(r: &OpResult) -> Option<FailCode> {
    r.error.as_ref().map(|e| e.code)
}

// ---------------------------------------------------------------- execute

#[tokio::test]
async fn same_volume_move_renames_and_records_hashes() {
    let env = Env::new().await;
    let src = env.path("in/a.txt");
    let dest = env.path("out/sub/a.txt");
    write(&src, b"alpha");
    let p = env.fake();

    let rep = execute(&[planned(&src, &dest, OpKind::Move)], &env.journal, &p)
        .await
        .unwrap();
    assert_eq!((rep.done_count(), rep.failed_count()), (1, 0));
    assert_eq!(rep.results[0].dest, dest);
    assert!(!src.exists());
    assert_eq!(fs::read(&dest).unwrap(), b"alpha");
    assert!(trashed(&env).is_empty());

    let op = env.op(rep.results[0].op_id).await;
    assert_eq!(op.state, OpState::Done);
    assert_eq!(op.op_type, OpType::Move);
    assert_eq!(op.hash_before.as_deref(), Some(hash(b"alpha").as_str()));
    assert_eq!(op.hash_after, op.hash_before);
    assert_eq!(op.size, Some(5));
    let batch = env.journal.get_batch(&rep.batch_id).await.unwrap().unwrap();
    assert_eq!(batch.kind, BatchKind::Apply);
    assert_eq!(batch.counts.done, 1);
    assert_eq!(batch.summary.as_deref(), Some("1 done, 0 failed"));
    assert!(batch.finished_at.is_some());
}

#[tokio::test]
async fn cross_volume_move_copies_verifies_and_trashes_source() {
    let env = Env::new().await;
    let src = env.path("in/b.bin");
    let dest = env.path("out/b.bin");
    let body: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    write(&src, &body);
    let mut p = env.fake();
    p.same_volume = false;

    let rep = execute(&[planned(&src, &dest, OpKind::Move)], &env.journal, &p)
        .await
        .unwrap();
    assert_eq!(code(&rep.results[0]), None);
    assert!(!src.exists());
    assert_eq!(fs::read(&dest).unwrap(), body);
    assert_eq!(trashed(&env), vec![body.clone()]);
    assert!(temp_files(&env.path("out")).is_empty());
    let op = env.op(rep.results[0].op_id).await;
    assert_eq!(op.hash_after.as_deref(), Some(hash(&body).as_str()));
}

#[tokio::test]
async fn copy_keeps_source_and_writes_identical_dest() {
    let env = Env::new().await;
    let src = env.path("in/c.txt");
    let dest = env.path("out/c.txt");
    write(&src, b"copy me");
    let p = env.fake();

    let rep = execute(&[planned(&src, &dest, OpKind::Copy)], &env.journal, &p)
        .await
        .unwrap();
    assert_eq!(code(&rep.results[0]), None);
    assert_eq!(fs::read(&src).unwrap(), b"copy me");
    assert_eq!(fs::read(&dest).unwrap(), b"copy me");
    assert!(trashed(&env).is_empty());
    assert!(temp_files(&env.path("out")).is_empty());
    assert_eq!(env.op(rep.results[0].op_id).await.op_type, OpType::Copy);
}

#[tokio::test]
async fn collision_at_execution_time_gets_next_free_name() {
    for (kind, same_volume) in [
        (OpKind::Move, true),
        (OpKind::Move, false),
        (OpKind::Copy, true),
    ] {
        let env = Env::new().await;
        let src = env.path("in/name.ext");
        let dest = env.path("out/name.ext");
        write(&src, b"mine");
        let op = planned(&src, &dest, kind);
        // Appears between preview and execution.
        write(&dest, b"theirs");
        let mut p = env.fake();
        p.same_volume = same_volume;

        let rep = execute(&[op], &env.journal, &p).await.unwrap();
        let second = env.path("out/name (2).ext");
        assert_eq!(code(&rep.results[0]), None, "{kind:?} {same_volume}");
        assert_eq!(rep.results[0].dest, second);
        assert_eq!(fs::read(&dest).unwrap(), b"theirs");
        assert_eq!(fs::read(&second).unwrap(), b"mine");
        assert_eq!(
            env.op(rep.results[0].op_id).await.dest_path,
            Some(second.clone())
        );
    }
}

#[tokio::test]
async fn changed_size_or_mtime_fails_without_touching() {
    let env = Env::new().await;
    let p = env.fake();
    let a = env.path("in/size.txt");
    let b = env.path("in/mtime.txt");
    write(&a, b"one");
    write(&b, b"two");
    let ops = [
        planned(&a, &env.path("out/size.txt"), OpKind::Move),
        planned(&b, &env.path("out/mtime.txt"), OpKind::Move),
    ];
    fs::write(&a, b"longer now").unwrap();
    let f = fs::File::options().write(true).open(&b).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
    drop(f);

    let rep = execute(&ops, &env.journal, &p).await.unwrap();
    for r in &rep.results {
        assert_eq!(code(r), Some(FailCode::ChangedSincePreview));
        let op = env.op(r.op_id).await;
        assert_eq!(op.state, OpState::Failed);
        assert!(op.error.unwrap().starts_with("ChangedSincePreview"));
        assert!(op.hash_before.is_none());
    }
    assert_eq!(fs::read(&a).unwrap(), b"longer now");
    assert_eq!(fs::read(&b).unwrap(), b"two");
    assert!(!env.path("out").exists());
}

#[tokio::test]
async fn missing_source_fails_and_batch_continues() {
    let env = Env::new().await;
    let p = env.fake();
    let gone = env.path("in/gone.txt");
    let ok = env.path("in/ok.txt");
    write(&gone, b"x");
    write(&ok, b"y");
    let ops = [
        planned(&gone, &env.path("out/gone.txt"), OpKind::Move),
        planned(&ok, &env.path("out/ok.txt"), OpKind::Move),
    ];
    p.trash(&gone).unwrap();

    let rep = execute(&ops, &env.journal, &p).await.unwrap();
    assert_eq!(code(&rep.results[0]), Some(FailCode::SourceMissing));
    assert_eq!(code(&rep.results[1]), None);
    assert_eq!(fs::read(env.path("out/ok.txt")).unwrap(), b"y");
    let batch = env.journal.get_batch(&rep.batch_id).await.unwrap().unwrap();
    assert_eq!((batch.counts.done, batch.counts.failed), (1, 1));
}

#[tokio::test]
async fn placeholder_is_never_hashed_or_moved() {
    let env = Env::new().await;
    let src = env.path("in/cloud.docx");
    write(&src, b"stub");
    let op = planned(&src, &env.path("out/cloud.docx"), OpKind::Move);
    let mut p = env.fake();
    p.placeholders.insert(src.clone());

    let rep = execute(&[op], &env.journal, &p).await.unwrap();
    assert_eq!(code(&rep.results[0]), Some(FailCode::CloudPlaceholder));
    let rec = env.op(rep.results[0].op_id).await;
    assert!(rec.hash_before.is_none());
    assert_eq!(rec.state, OpState::Failed);
    assert!(src.exists());
    assert!(!env.path("out").exists());
}

#[tokio::test]
async fn trash_op_fails_unsupported_and_touches_nothing() {
    let env = Env::new().await;
    let src = env.path("in/old.exe");
    write(&src, b"setup");
    let ok = env.path("in/ok.txt");
    write(&ok, b"ok");
    let p = env.fake();
    let ops = [
        planned(&src, Path::new(""), OpKind::Trash),
        planned(&ok, &env.path("out/ok.txt"), OpKind::Move),
    ];

    let rep = execute(&ops, &env.journal, &p).await.unwrap();
    let r = &rep.results[0];
    assert_eq!(code(r), Some(FailCode::Unsupported));
    assert_eq!(r.kind, OpKind::Trash);
    assert_eq!(r.dest, PathBuf::new());
    let rec = env.op(r.op_id).await;
    assert_eq!(rec.state, OpState::Failed);
    assert_eq!(rec.op_type, OpType::Trash);
    assert!(rec.dest_path.is_none());
    assert!(rec.hash_before.is_none(), "not even hashed");
    assert!(rec.error.unwrap().starts_with("Unsupported"));
    assert_eq!(fs::read(&src).unwrap(), b"setup");
    assert!(trashed(&env).is_empty());
    // The rest of the batch still runs.
    assert!(rep.results[1].error.is_none());
    assert_eq!(rep.done_count(), 1);
}

#[tokio::test]
async fn locked_source_is_skipped() {
    let env = Env::new().await;
    let src = env.path("in/locked.xlsx");
    write(&src, b"busy");
    let op = planned(&src, &env.path("out/locked.xlsx"), OpKind::Move);
    let mut p = env.fake();
    p.locked.insert(src.clone());

    let rep = execute(&[op], &env.journal, &p).await.unwrap();
    assert_eq!(code(&rep.results[0]), Some(FailCode::Locked));
    assert!(env.op(rep.results[0].op_id).await.hash_before.is_none());
    assert!(src.exists());
}

#[tokio::test]
async fn cross_volume_without_recycle_bin_touches_nothing() {
    for kind in [OpKind::Move, OpKind::Copy] {
        let env = Env::new().await;
        let src = env.path("in/n.txt");
        write(&src, b"stay");
        let op = planned(&src, &env.path("out/n.txt"), kind);
        let mut p = env.fake();
        p.same_volume = false;
        p.can_trash = false;

        let rep = execute(&[op], &env.journal, &p).await.unwrap();
        assert_eq!(
            code(&rep.results[0]),
            Some(FailCode::NoRecycleBin),
            "{kind:?}"
        );
        assert_eq!(fs::read(&src).unwrap(), b"stay");
        assert!(tree(&env.path("out")).is_empty());
        assert_eq!(env.op(rep.results[0].op_id).await.state, OpState::Failed);
    }
}

#[tokio::test]
async fn same_volume_move_needs_no_recycle_bin() {
    let env = Env::new().await;
    let src = env.path("in/r.txt");
    write(&src, b"r");
    let op = planned(&src, &env.path("out/r.txt"), OpKind::Move);
    let mut p = env.fake();
    p.can_trash = false;

    let rep = execute(&[op], &env.journal, &p).await.unwrap();
    assert_eq!(code(&rep.results[0]), None);
}

#[tokio::test]
async fn failed_source_trash_removes_the_copy_again() {
    let env = Env::new().await;
    let src = env.path("in/t.txt");
    let dest = env.path("out/t.txt");
    write(&src, b"keep me once");
    let op = planned(&src, &dest, OpKind::Move);
    let mut p = env.fake();
    p.same_volume = false;
    p.trash_fails.insert(src.clone());

    let rep = execute(&[op], &env.journal, &p).await.unwrap();
    assert_eq!(code(&rep.results[0]), Some(FailCode::TrashFailed));
    assert_eq!(fs::read(&src).unwrap(), b"keep me once");
    assert!(!dest.exists());
    assert!(temp_files(&env.path("out")).is_empty());
    // The copy went to the Recycle Bin, not deleted.
    assert_eq!(trashed(&env), vec![b"keep me once".to_vec()]);
    assert_eq!(env.op(rep.results[0].op_id).await.state, OpState::Failed);
}

#[test]
fn journal_text_has_code_and_detail() {
    assert_eq!(OpError::new(FailCode::Locked).journal_text(), "Locked");
    assert_eq!(
        OpError::with(FailCode::Io, "disk full").journal_text(),
        "Io: disk full"
    );
}

#[tokio::test]
async fn copy_with_hash_mismatch_trashes_temp_and_fails() {
    let env = Env::new().await;
    let src = env.path("in/v.txt");
    let dest = env.path("out/v.txt");
    write(&src, b"real bytes");
    fs::create_dir_all(env.path("out")).unwrap();
    let p = env.fake();
    let t = Transfer {
        op_id: 7,
        source: &src,
        dest: dest.clone(),
        hash: "not the hash",
        size: 10,
        on_collision: OnCollision::NextFreeName,
    };
    let err = copy_transfer(&t, true, &env.journal, &p)
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(err.code, FailCode::VerifyFailed);
    assert!(!dest.exists());
    assert!(temp_files(&env.path("out")).is_empty());
    assert_eq!(trashed(&env), vec![b"real bytes".to_vec()]);
    assert_eq!(fs::read(&src).unwrap(), b"real bytes");
}

// ---------------------------------------------------------------- recover

/// Journals an operation that a crash left `in_progress` (or `planned` when
/// `started` is false), with the hash of `body` as `hash_before`.
async fn interrupted(
    env: &Env,
    op_type: OpType,
    src: &Path,
    dest: &Path,
    body: &[u8],
    started: bool,
) -> (String, i64) {
    let b = env.journal.create_batch(BatchKind::Apply).await.unwrap();
    let id = env
        .journal
        .insert_planned(
            &b,
            0,
            &NewOp {
                op_type,
                source: src.to_path_buf(),
                dest: Some(dest.to_path_buf()),
                size: Some(body.len() as u64),
                undo_of: None,
            },
        )
        .await
        .unwrap();
    if started {
        env.journal.start(id, Some(&hash(body))).await.unwrap();
    }
    (b, id)
}

fn decision(rep: &RecoveryReport, op_id: i64) -> &RecoveryDecision {
    rep.decisions.iter().find(|d| d.op_id == op_id).unwrap()
}

#[tokio::test]
async fn recover_with_nothing_open_writes_no_batch() {
    let env = Env::new().await;
    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    assert_eq!(rep, RecoveryReport::default());
    assert!(env.journal.list_batches(10).await.unwrap().is_empty());
}

#[tokio::test]
async fn recover_planned_op_fails_and_closes_batch() {
    let env = Env::new().await;
    let src = env.path("in/p.txt");
    write(&src, b"p");
    let (b, id) = interrupted(
        &env,
        OpType::Move,
        &src,
        &env.path("out/p.txt"),
        b"p",
        false,
    )
    .await;

    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    let d = decision(&rep, id);
    assert_eq!(
        (d.previous_state, d.new_state),
        (OpState::Planned, OpState::Failed)
    );
    let op = env.op(id).await;
    assert!(op.error.unwrap().starts_with("Interrupted"));
    assert!(src.exists());
    assert_eq!(rep.closed_batches, vec![b.clone()]);
    let closed = env.journal.get_batch(&b).await.unwrap().unwrap();
    assert!(closed.finished_at.is_some());
    assert!(closed.summary.unwrap().starts_with("interrupted"));
    let rb = env
        .journal
        .get_batch(rep.batch_id.as_ref().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rb.kind, BatchKind::Recovery);
    assert!(rb.summary.unwrap().contains(&format!("operation {id} ")));
    // Idempotent: nothing left for a second run.
    assert_eq!(
        recover(&env.journal, &env.fake()).await.unwrap(),
        RecoveryReport::default()
    );
}

#[tokio::test]
async fn recover_move_already_at_destination_is_done() {
    let env = Env::new().await;
    let src = env.path("in/m.txt");
    let dest = env.path("out/m.txt");
    write(&dest, b"moved");
    let (_b, id) = interrupted(&env, OpType::Move, &src, &dest, b"moved", true).await;

    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    assert_eq!(decision(&rep, id).new_state, OpState::Done);
    let op = env.op(id).await;
    assert_eq!(op.hash_after.as_deref(), Some(hash(b"moved").as_str()));
    assert_eq!(fs::read(&dest).unwrap(), b"moved");
}

#[tokio::test]
async fn recover_move_copied_but_source_not_trashed_trashes_source() {
    let env = Env::new().await;
    let src = env.path("in/x.txt");
    let dest = env.path("out/x.txt");
    write(&src, b"both");
    write(&dest, b"both");
    let (_b, id) = interrupted(&env, OpType::Move, &src, &dest, b"both", true).await;

    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    assert_eq!(decision(&rep, id).new_state, OpState::Done);
    assert!(!src.exists());
    assert_eq!(fs::read(&dest).unwrap(), b"both");
    assert_eq!(trashed(&env), vec![b"both".to_vec()]);
    // The trash was journaled in the recovery batch.
    let ops = env
        .journal
        .list_ops(rep.batch_id.as_ref().unwrap())
        .await
        .unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(
        (ops[0].op_type, ops[0].state),
        (OpType::Trash, OpState::Done)
    );
    assert_eq!(ops[0].source_path, src);
}

#[tokio::test]
async fn recover_move_source_trash_failure_removes_copy() {
    let env = Env::new().await;
    let src = env.path("in/x.txt");
    let dest = env.path("out/x.txt");
    write(&src, b"both");
    write(&dest, b"both");
    let (_b, id) = interrupted(&env, OpType::Move, &src, &dest, b"both", true).await;
    let mut p = env.fake();
    p.trash_fails.insert(src.clone());

    let rep = recover(&env.journal, &p).await.unwrap();
    assert_eq!(decision(&rep, id).new_state, OpState::Failed);
    assert!(env.op(id).await.error.unwrap().starts_with("TrashFailed"));
    assert_eq!(fs::read(&src).unwrap(), b"both");
    assert!(!dest.exists());
}

#[tokio::test]
async fn recover_move_changed_source_keeps_source_and_trashes_copy() {
    let env = Env::new().await;
    let src = env.path("in/x.txt");
    let dest = env.path("out/x.txt");
    write(&src, b"edited after crash");
    write(&dest, b"orig");
    let (_b, id) = interrupted(&env, OpType::Move, &src, &dest, b"orig", true).await;

    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    assert_eq!(decision(&rep, id).new_state, OpState::Failed);
    assert_eq!(fs::read(&src).unwrap(), b"edited after crash");
    assert!(!dest.exists());
    assert_eq!(trashed(&env), vec![b"orig".to_vec()]);
}

#[tokio::test]
async fn recover_move_interrupted_mid_copy_trashes_temp_keeps_source() {
    let env = Env::new().await;
    let src = env.path("in/big.bin");
    let dest = env.path("out/big.bin");
    write(&src, b"full content");
    let (_b, id) = interrupted(&env, OpType::Move, &src, &dest, b"full content", true).await;
    let temp = temp_path(&env.path("out"), id);
    write(&temp, b"full");

    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    assert_eq!(decision(&rep, id).new_state, OpState::Failed);
    assert_eq!(fs::read(&src).unwrap(), b"full content");
    assert!(!temp.exists() && !dest.exists());
    assert_eq!(trashed(&env), vec![b"full".to_vec()]);
}

#[tokio::test]
async fn recover_move_not_started_on_disk_fails_without_cleanup() {
    let env = Env::new().await;
    let src = env.path("in/s.txt");
    write(&src, b"s");
    let (_b, id) = interrupted(&env, OpType::Move, &src, &env.path("out/s.txt"), b"s", true).await;

    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    assert_eq!(decision(&rep, id).new_state, OpState::Failed);
    assert!(src.exists());
    assert!(trashed(&env).is_empty());
    assert!(
        env.journal
            .list_ops(rep.batch_id.as_ref().unwrap())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn recover_leaves_foreign_destination_alone() {
    let env = Env::new().await;
    let src = env.path("in/f.txt");
    let dest = env.path("out/f.txt");
    write(&src, b"mine");
    write(&dest, b"someone else's");
    let (_b, id) = interrupted(&env, OpType::Move, &src, &dest, b"mine", true).await;

    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    assert_eq!(decision(&rep, id).new_state, OpState::Failed);
    assert_eq!(fs::read(&src).unwrap(), b"mine");
    assert_eq!(fs::read(&dest).unwrap(), b"someone else's");
    assert!(trashed(&env).is_empty());
}

#[tokio::test]
async fn recover_placeholder_source_is_not_trashed_or_hashed() {
    let env = Env::new().await;
    let src = env.path("in/c.txt");
    let dest = env.path("out/c.txt");
    write(&src, b"cloud");
    write(&dest, b"cloud");
    let (_b, id) = interrupted(&env, OpType::Move, &src, &dest, b"cloud", true).await;
    let mut p = env.fake();
    p.placeholders.insert(src.clone());

    let rep = recover(&env.journal, &p).await.unwrap();
    assert_eq!(decision(&rep, id).new_state, OpState::Failed);
    assert!(src.exists(), "placeholder source stays");
    assert!(!dest.exists(), "verified copy goes to the Recycle Bin");
}

#[tokio::test]
async fn recover_placeholder_destination_with_source_gone_is_done_unhashed() {
    let env = Env::new().await;
    let src = env.path("in/c.txt");
    let dest = env.path("out/c.txt");
    write(&dest, b"cloud");
    let (_b, id) = interrupted(&env, OpType::Move, &src, &dest, b"cloud", true).await;
    let mut p = env.fake();
    p.placeholders.insert(dest.clone());

    let rep = recover(&env.journal, &p).await.unwrap();
    assert_eq!(decision(&rep, id).new_state, OpState::Done);
    assert!(env.op(id).await.hash_after.is_none());
}

#[tokio::test]
async fn recover_copy_states() {
    let env = Env::new().await;
    let a = env.path("in/a.txt");
    let a_dest = env.path("out/a.txt");
    write(&a, b"a");
    write(&a_dest, b"a");
    let (_, done_id) = interrupted(&env, OpType::Copy, &a, &a_dest, b"a", true).await;

    let b = env.path("in/b.txt");
    let b_dest = env.path("out/b.txt");
    write(&b, b"bbbb");
    let (_, fail_id) = interrupted(&env, OpType::Copy, &b, &b_dest, b"bbbb", true).await;
    let temp = temp_path(&env.path("out"), fail_id);
    write(&temp, b"bb");

    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    assert_eq!(decision(&rep, done_id).new_state, OpState::Done);
    assert_eq!(decision(&rep, fail_id).new_state, OpState::Failed);
    assert_eq!(fs::read(&a).unwrap(), b"a");
    assert_eq!(fs::read(&a_dest).unwrap(), b"a");
    assert_eq!(fs::read(&b).unwrap(), b"bbbb");
    assert!(!temp.exists() && !b_dest.exists());
    assert_eq!(trashed(&env), vec![b"bb".to_vec()]);
}

#[tokio::test]
async fn recover_trash_op_by_disk_state() {
    let env = Env::new().await;
    let b = env.journal.create_batch(BatchKind::Recovery).await.unwrap();
    let still = env.path("still.txt");
    write(&still, b"s");
    let mut ids = vec![];
    for (seq, path) in [(0, still.clone()), (1, env.path("gone.txt"))] {
        let id = env
            .journal
            .insert_planned(
                &b,
                seq,
                &NewOp {
                    op_type: OpType::Trash,
                    source: path,
                    dest: None,
                    size: None,
                    undo_of: None,
                },
            )
            .await
            .unwrap();
        env.journal.start(id, None).await.unwrap();
        ids.push(id);
    }
    let rep = recover(&env.journal, &env.fake()).await.unwrap();
    assert_eq!(decision(&rep, ids[0]).new_state, OpState::Failed);
    assert_eq!(decision(&rep, ids[1]).new_state, OpState::Done);
    assert!(still.exists());
}

/// Every interrupted move ends with its file exactly once, at source or
/// destination (Recycle Bin aside).
#[tokio::test]
async fn recover_leaves_every_moved_file_exactly_once() {
    let env = Env::new().await;
    // (name, at source, at dest, temp, started)
    let cases: &[(&str, bool, bool, bool, bool)] = &[
        ("planned", true, false, false, false),
        ("not-started", true, false, false, true),
        ("mid-copy", true, false, true, true),
        ("placed", true, true, false, true),
        ("finished", false, true, false, true),
    ];
    let mut files = vec![];
    for (name, at_src, at_dest, temp, started) in cases {
        let body = format!("content of {name}").into_bytes();
        let src = env.path(&format!("in/{name}.txt"));
        let dest = env.path(&format!("out/{name}.txt"));
        if *at_src {
            write(&src, &body);
        }
        if *at_dest {
            write(&dest, &body);
        }
        let (_b, id) = interrupted(&env, OpType::Move, &src, &dest, &body, *started).await;
        if *temp {
            write(&temp_path(&env.path("out"), id), &body[..4]);
        }
        files.push((src, dest, body));
    }
    recover(&env.journal, &env.fake()).await.unwrap();
    for (src, dest, body) in &files {
        let copies = [src, dest]
            .iter()
            .filter(|p| fs::read(p).ok().as_deref() == Some(body.as_slice()))
            .count();
        assert_eq!(copies, 1, "{}", src.display());
    }
    assert!(temp_files(&env.path("out")).is_empty());
    assert!(env.journal.unfinished_ops().await.unwrap().is_empty());
}

// ---------------------------------------------------------------- undo

/// A small tree under `root/in` plus an unrelated file already in `root/out`.
fn undo_fixture(env: &Env) -> (PathBuf, Vec<PlannedOp>) {
    let root = env.path("root");
    write(&root.join("out/existing.txt"), b"was here before");
    let files: &[(&str, &str, OpKind, &[u8])] = &[
        ("in/a.pdf", "out/docs/a.pdf", OpKind::Move, b"pdf bytes"),
        ("in/sub/b.jpg", "out/img/b.jpg", OpKind::Move, b"jpg bytes"),
        ("in/c.txt", "out/copies/c.txt", OpKind::Copy, b"text"),
        ("in/sub/deep/d.zip", "out/z/d.zip", OpKind::Move, b"zip"),
    ];
    let ops = files
        .iter()
        .map(|(src, dest, kind, body)| {
            let s = root.join(src);
            write(&s, body);
            planned(&s, &root.join(dest), *kind)
        })
        .collect();
    (root, ops)
}

#[tokio::test]
async fn undo_restores_byte_identical_tree() {
    for same_volume in [true, false] {
        let env = Env::new().await;
        let (root, ops) = undo_fixture(&env);
        let before = tree(&root);
        let mut p = env.fake();
        p.same_volume = same_volume;

        let rep = execute(&ops, &env.journal, &p).await.unwrap();
        assert_eq!(rep.failed_count(), 0);
        assert_ne!(tree(&root), before);

        let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
            .await
            .unwrap();
        assert_eq!(u.undone.len(), 4, "{same_volume}");
        assert!(u.needs_confirmation.is_empty() && u.failed.is_empty() && u.skipped.is_empty());
        assert_eq!(tree(&root), before, "same_volume={same_volume}");

        // Undone last-first; originals rolled back, undo ops point at them.
        let ids: Vec<i64> = rep.results.iter().rev().map(|r| r.op_id).collect();
        assert_eq!(u.undone, ids);
        for r in &rep.results {
            assert_eq!(env.op(r.op_id).await.state, OpState::RolledBack);
        }
        let ub = u.undo_batch_id.unwrap();
        let undo_ops = env.journal.list_ops(&ub).await.unwrap();
        assert_eq!(
            undo_ops
                .iter()
                .map(|o| o.undo_of.unwrap())
                .collect::<Vec<_>>(),
            ids
        );
        assert!(undo_ops.iter().all(|o| o.state == OpState::Done));
        assert_eq!(
            env.journal.get_batch(&ub).await.unwrap().unwrap().kind,
            BatchKind::Undo
        );
        // The copy was undone through the Recycle Bin, not deleted.
        assert!(trashed(&env).contains(&b"text".to_vec()));

        // A second undo finds nothing left to do and opens no batch.
        let again = undo_batch(&rep.batch_id, &env.journal, &p, false)
            .await
            .unwrap();
        assert!(again.undone.is_empty() && again.undo_batch_id.is_none());
        assert_eq!(again.skipped.len(), 4);
    }
}

#[tokio::test]
async fn undo_with_modified_destination_needs_confirmation() {
    let env = Env::new().await;
    let src = env.path("in/m.txt");
    let dest = env.path("out/m.txt");
    write(&src, b"original");
    let p = env.fake();
    let rep = execute(&[planned(&src, &dest, OpKind::Move)], &env.journal, &p)
        .await
        .unwrap();
    fs::write(&dest, b"user edited").unwrap();

    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert!(u.undone.is_empty());
    assert_eq!(u.needs_confirmation.len(), 1);
    assert_eq!(
        u.needs_confirmation[0].reasons,
        vec![ConfirmReason::Modified]
    );
    assert_eq!(fs::read(&dest).unwrap(), b"user edited");
    assert!(!src.exists());
    assert!(u.undo_batch_id.is_none());
    assert_eq!(env.op(rep.results[0].op_id).await.state, OpState::Done);

    let forced = undo_batch(&rep.batch_id, &env.journal, &p, true)
        .await
        .unwrap();
    assert_eq!(forced.undone, vec![rep.results[0].op_id]);
    assert_eq!(fs::read(&src).unwrap(), b"user edited");
    assert!(!dest.exists());
}

#[tokio::test]
async fn undo_with_occupied_source_needs_confirmation() {
    let env = Env::new().await;
    let src = env.path("in/o.txt");
    let dest = env.path("out/o.txt");
    write(&src, b"moved away");
    let p = env.fake();
    let rep = execute(&[planned(&src, &dest, OpKind::Move)], &env.journal, &p)
        .await
        .unwrap();
    write(&src, b"new file, same name");

    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert_eq!(
        u.needs_confirmation[0].reasons,
        vec![ConfirmReason::SourceOccupied]
    );
    assert_eq!(fs::read(&src).unwrap(), b"new file, same name");
    assert_eq!(fs::read(&dest).unwrap(), b"moved away");

    // Confirmed: restored beside it, never over it.
    let forced = undo_batch(&rep.batch_id, &env.journal, &p, true)
        .await
        .unwrap();
    let beside = env.path("in/o (2).txt");
    assert_eq!(
        forced.restored_as,
        vec![Restored {
            op_id: rep.results[0].op_id,
            path: beside.clone()
        }]
    );
    assert_eq!(fs::read(&src).unwrap(), b"new file, same name");
    assert_eq!(fs::read(&beside).unwrap(), b"moved away");
    assert!(!dest.exists());
}

#[tokio::test]
async fn undo_skips_failed_ops_and_lists_them() {
    let env = Env::new().await;
    let ok = env.path("in/ok.txt");
    let locked = env.path("in/locked.txt");
    write(&ok, b"ok");
    write(&locked, b"locked");
    let mut p = env.fake();
    p.locked.insert(locked.clone());
    let rep = execute(
        &[
            planned(&ok, &env.path("out/ok.txt"), OpKind::Move),
            planned(&locked, &env.path("out/locked.txt"), OpKind::Move),
        ],
        &env.journal,
        &p,
    )
    .await
    .unwrap();

    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert_eq!(u.undone, vec![rep.results[0].op_id]);
    assert_eq!(
        u.skipped,
        vec![SkippedOp {
            op_id: rep.results[1].op_id,
            state: OpState::Failed
        }]
    );
    assert!(ok.exists() && locked.exists());
}

#[tokio::test]
async fn undo_copy_with_original_gone_needs_confirmation() {
    let env = Env::new().await;
    let src = env.path("in/c.txt");
    let dest = env.path("out/c.txt");
    write(&src, b"c");
    let p = env.fake();
    let rep = execute(&[planned(&src, &dest, OpKind::Copy)], &env.journal, &p)
        .await
        .unwrap();
    p.trash(&src).unwrap();

    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert_eq!(
        u.needs_confirmation[0].reasons,
        vec![ConfirmReason::OriginalMissing]
    );
    assert!(dest.exists());
}

#[tokio::test]
async fn undo_cross_volume_without_recycle_bin_fails_untouched() {
    let env = Env::new().await;
    let src = env.path("in/x.txt");
    let dest = env.path("out/x.txt");
    write(&src, b"x");
    let mut p = env.fake();
    p.same_volume = false;
    let rep = execute(&[planned(&src, &dest, OpKind::Move)], &env.journal, &p)
        .await
        .unwrap();
    p.can_trash = false;

    let u = undo_batch(&rep.batch_id, &env.journal, &p, false)
        .await
        .unwrap();
    assert_eq!(u.failed[0].error.code, FailCode::NoRecycleBin);
    assert_eq!(fs::read(&dest).unwrap(), b"x");
    assert!(!src.exists());
}

#[tokio::test]
async fn undo_rejects_unknown_open_and_non_apply_batches() {
    let env = Env::new().await;
    let p = env.fake();
    assert!(
        undo_batch("missing", &env.journal, &p, false)
            .await
            .is_err()
    );
    let open = env.journal.create_batch(BatchKind::Apply).await.unwrap();
    assert!(undo_batch(&open, &env.journal, &p, false).await.is_err());
    let undo = env.journal.create_batch(BatchKind::Undo).await.unwrap();
    env.journal.finish_batch(&undo, "x").await.unwrap();
    assert!(undo_batch(&undo, &env.journal, &p, false).await.is_err());
}

#[tokio::test]
async fn recover_interrupted_undo_rolls_back_original() {
    let env = Env::new().await;
    let src = env.path("in/u.txt");
    let dest = env.path("out/u.txt");
    write(&src, b"u");
    let p = env.fake();
    let rep = execute(&[planned(&src, &dest, OpKind::Move)], &env.journal, &p)
        .await
        .unwrap();
    let orig = rep.results[0].op_id;
    // Crash right after the undo rename, before the journal said done.
    let ub = env.journal.create_batch(BatchKind::Undo).await.unwrap();
    let id = env
        .journal
        .insert_planned(
            &ub,
            0,
            &NewOp {
                op_type: OpType::Move,
                source: dest.clone(),
                dest: Some(src.clone()),
                size: Some(1),
                undo_of: Some(orig),
            },
        )
        .await
        .unwrap();
    env.journal.start(id, Some(&hash(b"u"))).await.unwrap();
    p.rename_no_replace(&dest, &src).unwrap();

    let r = recover(&env.journal, &p).await.unwrap();
    assert_eq!(decision(&r, id).new_state, OpState::Done);
    assert_eq!(env.op(orig).await.state, OpState::RolledBack);
    assert_eq!(fs::read(&src).unwrap(), b"u");
}
