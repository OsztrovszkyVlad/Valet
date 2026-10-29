//! Upgrade from an earlier release with an existing journal (PLAN.md Phase 3).
//!
//! Each test builds the database an earlier release left behind, without the
//! current storage or journal code: the migrations as they shipped (copies in
//! `tests/fixtures/historical_migrations/`, taken from the Phase 2 report
//! commit d36cc51) are run by a runtime `sqlx` migrator, exactly as the old
//! `Db::connect` did through `sqlx::migrate!`, so `_sqlx_migrations` holds the
//! old versions and checksums. Rows are inserted with plain SQL matching
//! those schemas, and the files are placed where the journal says.
//!
//! - Phase 1 (migrations 0001-0003): an apply batch with done moves and
//!   failed operations, an older apply batch already undone (undo batch with
//!   `undo_of` links), `operation_history` statistics, index and rule rows.
//! - Phase 2 (0001-0004): the same, plus an apply batch with a `trash`
//!   operation undone by a `restore` operation.
//!
//! The current `Db::connect` then upgrades each database. Checked: all
//! current migrations recorded, journal/index/rule rows unchanged, the
//! `operation_history` table and its indexes gone, the schema equal to a new
//! database's, nothing to recover, a second open changes nothing, and the
//! Phase-1-era apply batch undoes with the current `undo_batch` to a
//! byte-identical tree.

use std::{
    fs,
    path::{Path, PathBuf},
};

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use valet_core::{
    executor::{self, SkippedOp},
    journal::{OpState, OpType},
    platform::testing::FakePlatform,
    storage::Db,
};

/// Schema version the release left: 3 = Phase 1, 4 = Phase 2.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Era {
    Phase1,
    Phase2,
}

impl Era {
    fn last_migration(self) -> i64 {
        match self {
            Era::Phase1 => 3,
            Era::Phase2 => 4,
        }
    }
}

fn blake3_file(path: &Path) -> String {
    blake3::hash(&fs::read(path).unwrap()).to_hex().to_string()
}

/// Sorted (path relative to `root` with `/`, blake3) of every file below `root`.
fn tree(root: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).unwrap();
                let rel: Vec<String> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                out.push((rel.join("/"), blake3_file(&path)));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Every row of `table` as one string per row (`quote()` keeps NULL and
/// types apart), in `order`.
async fn dump(pool: &SqlitePool, table: &str, columns: &[&str], order: &str) -> Vec<String> {
    let expr = columns
        .iter()
        .map(|c| format!("quote({c})"))
        .collect::<Vec<_>>()
        .join(" || ',' || ");
    sqlx::query_scalar(&format!("SELECT {expr} FROM {table} ORDER BY {order}"))
        .fetch_all(pool)
        .await
        .unwrap()
}

const OP_COLUMNS: &[&str] = &[
    "id",
    "batch_id",
    "seq",
    "op_type",
    "source_path",
    "dest_path",
    "size",
    "hash_before",
    "hash_after",
    "state",
    "error",
    "created_at",
    "finished_at",
    "undo_of",
];

/// Journal, index and rule rows, compared before and after the upgrade.
async fn data(pool: &SqlitePool) -> Vec<Vec<String>> {
    vec![
        dump(
            pool,
            "batches",
            &["id", "kind", "created_at", "finished_at", "summary"],
            "rowid",
        )
        .await,
        dump(pool, "operations", OP_COLUMNS, "id").await,
        dump(
            pool,
            "files",
            &["id", "path", "size", "mtime", "hash_short", "tags_json"],
            "rowid",
        )
        .await,
        dump(
            pool,
            "rules",
            &["id", "json", "version", "enabled"],
            "rowid",
        )
        .await,
    ]
}

/// Tables, indexes and their SQL, without SQLite's internal tables.
async fn schema(pool: &SqlitePool) -> Vec<String> {
    dump(
        pool,
        "sqlite_master",
        &["type", "name", "tbl_name", "sql"],
        "type, name",
    )
    .await
    .into_iter()
    .filter(|row| !row.contains("'sqlite_"))
    .collect()
}

async fn migrations(pool: &SqlitePool) -> Vec<String> {
    dump(
        pool,
        "_sqlx_migrations",
        &[
            "version",
            "description",
            "installed_on",
            "success",
            "hex(checksum)",
        ],
        "version",
    )
    .await
}

async fn operations_seq(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT seq FROM sqlite_sequence WHERE name = 'operations'")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// An old database, the files on disk as the journal describes them, and the
/// ids the assertions need.
struct OldInstall {
    tmp: tempfile::TempDir,
    home: PathBuf,
    db_path: PathBuf,
    /// The tree before the batch to undo ran.
    original: Vec<(String, String)>,
    /// Phase-1-era apply batch with done and failed operations.
    apply: String,
    /// Its done moves, in seq order.
    done_moves: Vec<i64>,
    /// Its failed operations, in seq order.
    failed: Vec<i64>,
    /// Older apply batch, rolled back by `undo`.
    undone_apply: String,
    undone_op: i64,
    undo: String,
    undo_op: i64,
    /// Phase 2 only: trash batch, its trash op, the undo batch and its restore op.
    trash: Option<(String, i64, String, i64)>,
}

struct Writer<'a> {
    pool: &'a SqlitePool,
}

impl Writer<'_> {
    async fn batch(&self, id: &str, kind: &str, created: &str, finished: &str, summary: &str) {
        sqlx::query(
            "INSERT INTO batches (id, kind, created_at, finished_at, summary) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(id)
        .bind(kind)
        .bind(created)
        .bind(finished)
        .bind(summary)
        .execute(self.pool)
        .await
        .unwrap();
    }

    /// Inserts one operation row in its final state, as the old journal left it.
    #[allow(clippy::too_many_arguments)]
    async fn op(
        &self,
        batch: &str,
        seq: i64,
        op_type: &str,
        source: &Path,
        dest: Option<&Path>,
        size: Option<i64>,
        hash_before: Option<&str>,
        hash_after: Option<&str>,
        state: &str,
        error: Option<&str>,
        at: &str,
        undo_of: Option<i64>,
    ) -> i64 {
        sqlx::query(
            "INSERT INTO operations (batch_id, seq, op_type, source_path, dest_path, size,
                                     hash_before, hash_after, state, error, created_at,
                                     finished_at, undo_of)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?12)",
        )
        .bind(batch)
        .bind(seq)
        .bind(op_type)
        .bind(text(source))
        .bind(dest.map(text))
        .bind(size)
        .bind(hash_before)
        .bind(hash_after)
        .bind(state)
        .bind(error)
        .bind(at)
        .bind(undo_of)
        .execute(self.pool)
        .await
        .unwrap()
        .last_insert_rowid()
    }
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Builds the database and files an `era` release would have left.
async fn old_install(era: Era) -> OldInstall {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let db_path = tmp.path().join("appdata").join("valet.sqlite3");
    fs::create_dir_all(db_path.parent().unwrap()).unwrap();

    // Files before the batch to undo ran.
    let dl = home.join("Downloads");
    let invoice = dl.join("factura 2026.pdf");
    let photo = dl.join("photo.jpg");
    let empty = dl.join("empty.txt");
    let locked = dl.join("report.docx");
    let gone = dl.join("gone.zip");
    let old_note = home.join("old").join("note.txt");
    let unrelated_photo = home.join("Pictures").join("photo.jpg");
    let big: Vec<u8> = (0..200_000u32).map(|i| (i * 31 % 251) as u8).collect();
    write(&invoice, &big);
    write(&photo, b"\xff\xd8\xff\xe0 jpeg bytes");
    write(&empty, b"");
    write(&locked, b"docx held by Word");
    write(&old_note, b"moved and moved back in Phase 1");
    write(&unrelated_photo, b"a different photo, there before");
    if era == Era::Phase2 {
        write(
            &home.join("Desktop").join("old.log"),
            b"trashed then restored",
        );
    }
    let original = tree(&home);

    // The old Db::connect: same options, migrations as they shipped.
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal),
        )
        .await
        .unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/historical_migrations");
    let mut old = Migrator::new(dir).await.unwrap();
    let last = era.last_migration();
    old.migrations = std::borrow::Cow::Owned(
        old.migrations
            .iter()
            .filter(|m| m.version <= last)
            .cloned()
            .collect(),
    );
    assert_eq!(old.migrations.len() as i64, last);
    old.run(&pool).await.unwrap();

    let w = Writer { pool: &pool };

    // Index and rule rows (raw; compared byte for byte after the upgrade).
    sqlx::query(
        "INSERT INTO files (id, path, size, mtime, hash_short, tags_json)
         VALUES ('f1', ?1, 200000, 1761000000, 'abcd1234', '[]')",
    )
    .bind(text(&invoice))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO rules (id, json, version, enabled)
         VALUES ('00000000-0000-0000-0000-000000000001', '{\"name\":\"Facturi\"}', 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Phase 1 statistics, dropped by migration 0005.
    for (src, status, err) in [
        (&invoice, "success", None),
        (&photo, "success", None),
        (&locked, "failed", Some("Locked")),
    ] {
        sqlx::query(
            "INSERT INTO operation_history (source_path, destination_path, operation_type, rule_name, status, error_message)
             VALUES (?1, 'x', 'move', 'Facturi', ?2, ?3)",
        )
        .bind(text(src))
        .bind(status)
        .bind(err)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Older apply batch, undone in Phase 1: the note is back at its source.
    let archived = home.join("Archive").join("note.txt");
    let note_hash = blake3_file(&old_note);
    let undone_apply = "11111111-1111-4111-8111-111111111111".to_string();
    let undo = "22222222-2222-4222-8222-222222222222".to_string();
    w.batch(
        &undone_apply,
        "apply",
        "2026-10-20T09:00:00.000Z",
        "2026-10-20T09:00:01.000Z",
        "1 done, 0 failed",
    )
    .await;
    let size = Some(fs::metadata(&old_note).unwrap().len() as i64);
    let undone_op = w
        .op(
            &undone_apply,
            0,
            "move",
            &old_note,
            Some(&archived),
            size,
            Some(&note_hash),
            Some(&note_hash),
            "rolled_back",
            None,
            "2026-10-20T09:00:00.500Z",
            None,
        )
        .await;
    w.batch(
        &undo,
        "undo",
        "2026-10-20T09:05:00.000Z",
        "2026-10-20T09:05:01.000Z",
        "undo of batch 11111111-1111-4111-8111-111111111111: 1 undone, 0 need confirmation, 0 failed, 0 skipped",
    )
    .await;
    let undo_op = w
        .op(
            &undo,
            0,
            "move",
            &archived,
            Some(&old_note),
            size,
            Some(&note_hash),
            Some(&note_hash),
            "done",
            None,
            "2026-10-20T09:05:00.500Z",
            Some(undone_op),
        )
        .await;

    // The apply batch to undo now: three done moves (one renamed on
    // collision, one empty file) and two failed operations.
    let apply = "33333333-3333-4333-8333-333333333333".to_string();
    w.batch(
        &apply,
        "apply",
        "2026-10-21T10:00:00.000Z",
        "2026-10-21T10:00:02.000Z",
        "3 done, 2 failed",
    )
    .await;
    let moves = [
        (
            &invoice,
            home.join("Documents")
                .join("Facturi")
                .join("factura 2026.pdf"),
        ),
        (&photo, home.join("Pictures").join("photo (2).jpg")),
        (&empty, home.join("Documents").join("empty.txt")),
    ];
    let mut done_moves = Vec::new();
    for (seq, (src, dst)) in moves.iter().enumerate() {
        let h = blake3_file(src);
        let size = fs::metadata(src).unwrap().len() as i64;
        let id = w
            .op(
                &apply,
                seq as i64,
                "move",
                src,
                Some(dst),
                Some(size),
                Some(&h),
                Some(&h),
                "done",
                None,
                &format!("2026-10-21T10:00:0{seq}.100Z"),
                None,
            )
            .await;
        fs::create_dir_all(dst.parent().unwrap()).unwrap();
        fs::rename(src, dst).unwrap();
        done_moves.push(id);
    }
    let docs = home.join("Documents");
    let failed = vec![
        w.op(
            &apply,
            3,
            "move",
            &locked,
            Some(&docs.join("report.docx")),
            Some(17),
            None,
            None,
            "failed",
            Some("Locked"),
            "2026-10-21T10:00:01.500Z",
            None,
        )
        .await,
        w.op(
            &apply,
            4,
            "move",
            &gone,
            Some(&docs.join("gone.zip")),
            Some(5),
            None,
            None,
            "failed",
            Some("SourceMissing: file is gone"),
            "2026-10-21T10:00:01.600Z",
            None,
        )
        .await,
    ];

    // Phase 2: a trash operation, undone by a restore from the Recycle Bin.
    let trash = if era == Era::Phase2 {
        let log = home.join("Desktop").join("old.log");
        let h = blake3_file(&log);
        let size = Some(fs::metadata(&log).unwrap().len() as i64);
        let tb = "44444444-4444-4444-8444-444444444444".to_string();
        let ub = "55555555-5555-4555-8555-555555555555".to_string();
        w.batch(
            &tb,
            "apply",
            "2026-10-22T08:00:00.000Z",
            "2026-10-22T08:00:01.000Z",
            "1 done, 0 failed",
        )
        .await;
        let trash_op = w
            .op(
                &tb,
                0,
                "trash",
                &log,
                None,
                size,
                Some(&h),
                None,
                "rolled_back",
                None,
                "2026-10-22T08:00:00.200Z",
                None,
            )
            .await;
        w.batch(
            &ub,
            "undo",
            "2026-10-22T08:10:00.000Z",
            "2026-10-22T08:10:01.000Z",
            "undo of batch 44444444-4444-4444-8444-444444444444: 1 undone, 0 need confirmation, 0 failed, 0 skipped",
        )
        .await;
        let restore_op = w
            .op(
                &ub,
                0,
                "restore",
                &log,
                Some(&log),
                size,
                Some(&h),
                Some(&h),
                "done",
                None,
                "2026-10-22T08:10:00.200Z",
                Some(trash_op),
            )
            .await;
        Some((tb, trash_op, ub, restore_op))
    } else {
        None
    };

    pool.close().await;
    OldInstall {
        tmp,
        home,
        db_path,
        original,
        apply,
        done_moves,
        failed,
        undone_apply,
        undone_op,
        undo,
        undo_op,
        trash,
    }
}

async fn upgrade_and_undo(era: Era) {
    let old = old_install(era).await;

    // Snapshot of the old database, read with a plain pool.
    let pool = SqlitePoolOptions::new()
        .connect_with(SqliteConnectOptions::new().filename(&old.db_path))
        .await
        .unwrap();
    let before = data(&pool).await;
    let seq_before = operations_seq(&pool).await;
    let old_versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(old_versions, (1..=era.last_migration()).collect::<Vec<_>>());
    let history_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM operation_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(history_rows, 3);
    pool.close().await;
    let ops_before = before[1].len();
    assert_eq!(ops_before, if era == Era::Phase2 { 9 } else { 7 });

    // Upgrade with the current code.
    let db = Db::connect(&old.db_path).await.unwrap();
    let current = sqlx::migrate!("./migrations");
    let latest = current.migrations.iter().map(|m| m.version).max().unwrap();
    assert!(latest >= 5, "latest migration {latest}");
    let applied: Vec<(i64, bool, Vec<u8>)> =
        sqlx::query_as("SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    let expected: Vec<(i64, bool, Vec<u8>)> = current
        .migrations
        .iter()
        .map(|m| (m.version, true, m.checksum.to_vec()))
        .collect();
    assert_eq!(applied, expected, "{era:?}: migrations after upgrade");

    // Rows unchanged, statistics table and its indexes gone.
    assert_eq!(data(&db.pool).await, before, "{era:?}: rows after upgrade");
    assert_eq!(operations_seq(&db.pool).await, seq_before);
    let left: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE name LIKE '%operation_history%'")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert!(left.is_empty(), "{era:?}: {left:?}");
    let fresh = Db::connect(&old.tmp.path().join("fresh.sqlite3"))
        .await
        .unwrap();
    let upgraded_schema = schema(&db.pool).await;
    assert_eq!(
        upgraded_schema,
        schema(&fresh.pool).await,
        "{era:?}: schema differs from a new database"
    );
    fresh.pool.close().await;

    // The current journal API reads the old rows.
    let j = db.journal();
    let batches = j.list_batches(100).await.unwrap();
    assert_eq!(batches.len(), if era == Era::Phase2 { 5 } else { 3 });
    let info = j.get_batch(&old.apply).await.unwrap().unwrap();
    assert_eq!(
        (
            info.counts.done,
            info.counts.failed,
            info.counts.rolled_back
        ),
        (3, 2, 0)
    );
    assert_eq!(
        info.finished_at.as_deref(),
        Some("2026-10-21T10:00:02.000Z")
    );
    let ops = j.list_ops(&old.apply).await.unwrap();
    let ids: Vec<i64> = ops.iter().map(|o| o.id).collect();
    assert_eq!(ids, [old.done_moves.clone(), old.failed.clone()].concat());
    for op in &ops[..3] {
        assert_eq!(op.op_type, OpType::Move);
        assert_eq!(op.state, OpState::Done);
        let dest = op.dest_path.as_ref().unwrap();
        assert_eq!(op.hash_after.as_deref(), Some(blake3_file(dest).as_str()));
        assert_eq!(op.hash_before, op.hash_after);
        assert!(!op.source_path.exists());
    }
    assert_eq!(ops[3].state, OpState::Failed);
    assert_eq!(ops[3].error.as_deref(), Some("Locked"));
    assert_eq!(ops[3].hash_before, None);
    assert_eq!(ops[4].error.as_deref(), Some("SourceMissing: file is gone"));
    let undone = j.get_op(old.undone_op).await.unwrap().unwrap();
    assert_eq!(undone.state, OpState::RolledBack);
    assert_eq!(undone.batch_id, old.undone_apply);
    let undo_op = j.get_op(old.undo_op).await.unwrap().unwrap();
    assert_eq!(undo_op.batch_id, old.undo);
    assert_eq!(undo_op.undo_of, Some(old.undone_op));
    assert_eq!(undo_op.state, OpState::Done);
    if let Some((tb, trash_op, ub, restore_op)) = &old.trash {
        let t = j.list_ops(tb).await.unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].id, *trash_op);
        assert_eq!(
            (t[0].op_type, t[0].state, t[0].dest_path.as_ref()),
            (OpType::Trash, OpState::RolledBack, None)
        );
        let r = j.list_ops(ub).await.unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, *restore_op);
        assert_eq!(
            (r[0].op_type, r[0].state, r[0].undo_of),
            (OpType::Restore, OpState::Done, Some(*trash_op))
        );
        assert_eq!(r[0].hash_before, t[0].hash_before);
    }

    // Startup recovery finds nothing to do.
    let platform = FakePlatform::with_trash_dir(old.tmp.path().join("bin"));
    let rec = executor::recover(&j, &platform).await.unwrap();
    assert_eq!(rec.batch_id, None, "{era:?}: {rec:?}");
    assert!(rec.decisions.is_empty() && rec.closed_batches.is_empty());

    // A second open changes nothing.
    let migrations_once = migrations(&db.pool).await;
    db.pool.close().await;
    let db = Db::connect(&old.db_path).await.unwrap();
    assert_eq!(migrations(&db.pool).await, migrations_once);
    assert_eq!(
        data(&db.pool).await,
        before,
        "{era:?}: rows after second open"
    );
    assert_eq!(schema(&db.pool).await, upgraded_schema);

    // The Phase-1-era apply batch undoes with the current code.
    let j = db.journal();
    let report = executor::undo_batch(&old.apply, &j, &platform, false)
        .await
        .unwrap();
    let mut expect_undone = old.done_moves.clone();
    expect_undone.reverse();
    assert_eq!(report.undone, expect_undone, "{era:?}: {report:?}");
    let mut expect_skipped: Vec<SkippedOp> = old
        .failed
        .iter()
        .map(|&op_id| SkippedOp {
            op_id,
            state: OpState::Failed,
        })
        .collect();
    expect_skipped.reverse();
    assert_eq!(report.skipped, expect_skipped);
    assert!(report.needs_confirmation.is_empty(), "{report:?}");
    assert!(report.failed.is_empty(), "{report:?}");
    assert!(report.restored_as.is_empty() && report.restored_modified.is_empty());
    let ub = report.undo_batch_id.clone().unwrap();
    let undo_ops = j.list_ops(&ub).await.unwrap();
    assert_eq!(
        undo_ops.iter().map(|o| o.undo_of).collect::<Vec<_>>(),
        expect_undone.iter().map(|&id| Some(id)).collect::<Vec<_>>()
    );
    for o in &undo_ops {
        assert_eq!((o.op_type, o.state), (OpType::Move, OpState::Done));
        // AUTOINCREMENT carried over: no id is reused.
        assert!(o.id > seq_before, "op id {} <= {seq_before}", o.id);
    }
    for op in j.list_ops(&old.apply).await.unwrap() {
        let want = if old.done_moves.contains(&op.id) {
            OpState::RolledBack
        } else {
            OpState::Failed
        };
        assert_eq!(op.state, want);
    }
    assert_eq!(tree(&old.home), old.original, "{era:?}: tree after undo");

    // The already undone batch has nothing left to undo.
    let again = executor::undo_batch(&old.undone_apply, &j, &platform, false)
        .await
        .unwrap();
    assert!(again.undone.is_empty() && again.undo_batch_id.is_none());
    assert_eq!(
        again.skipped,
        vec![SkippedOp {
            op_id: old.undone_op,
            state: OpState::RolledBack,
        }]
    );
    assert_eq!(tree(&old.home), old.original);
    db.pool.close().await;
}

#[tokio::test]
async fn phase1_database_upgrades_and_its_batch_undoes() {
    upgrade_and_undo(Era::Phase1).await;
}

#[tokio::test]
async fn phase2_database_upgrades_and_its_batch_undoes() {
    upgrade_and_undo(Era::Phase2).await;
}
