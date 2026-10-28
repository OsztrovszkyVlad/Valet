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
