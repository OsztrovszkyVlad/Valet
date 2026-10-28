//! Write-ahead journal of file operations (PLAN.md 4.2), stored in SQLite (WAL).
//!
//! Every operation row moves through
//! `planned -> in_progress -> done | failed`, then optionally `done -> rolled_back`
//! (also `planned -> failed` when an operation never starts). Each state
//! change is its own transaction; a batch is never one transaction.

use std::{fmt, path::Path, path::PathBuf, str::FromStr};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};
use uuid::Uuid;

/// SQL expression for "now" as an ISO-8601 UTC timestamp with milliseconds.
const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchKind {
    Apply,
    Undo,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpType {
    Move,
    Copy,
    Trash,
    /// Undo of a `trash` operation: the item comes back from the Recycle Bin
    /// to `source_path` (also stored as `dest_path`). `hash_before` holds the
    /// content hash expected after the restore.
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpState {
    Planned,
    InProgress,
    Done,
    Failed,
    RolledBack,
}

macro_rules! str_enum {
    ($t:ty, $($v:ident => $s:literal),+ $(,)?) => {
        impl $t {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$v => $s),+ }
            }
        }
        impl FromStr for $t {
            type Err = anyhow::Error;
            fn from_str(s: &str) -> Result<Self> {
                match s {
                    $($s => Ok(Self::$v),)+
                    other => Err(anyhow!("unknown {} value {other:?}", stringify!($t))),
                }
            }
        }
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

str_enum!(BatchKind, Apply => "apply", Undo => "undo", Recovery => "recovery");
str_enum!(OpType, Move => "move", Copy => "copy", Trash => "trash", Restore => "restore");
str_enum!(OpState,
    Planned => "planned",
    InProgress => "in_progress",
    Done => "done",
    Failed => "failed",
    RolledBack => "rolled_back",
);

impl OpState {
    pub const ALL: [OpState; 5] = [
        OpState::Planned,
        OpState::InProgress,
        OpState::Done,
        OpState::Failed,
        OpState::RolledBack,
    ];

    /// The journal's state machine. `failed` and `rolled_back` are final.
    pub fn can_transition_to(self, to: OpState) -> bool {
        use OpState::*;
        matches!(
            (self, to),
            (Planned, InProgress)
                | (Planned, Failed)
                | (InProgress, Done)
                | (InProgress, Failed)
                | (Done, RolledBack)
        )
    }

    fn is_final_outcome(self) -> bool {
        matches!(self, OpState::Done | OpState::Failed)
    }
}

/// Errors the journal reports for misuse (as opposed to SQLite failures).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalError {
    InvalidTransition {
        op_id: i64,
        from: OpState,
        to: OpState,
    },
    OpNotFound(i64),
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalError::InvalidTransition { op_id, from, to } => {
                write!(f, "operation {op_id}: invalid transition {from} -> {to}")
            }
            JournalError::OpNotFound(id) => write!(f, "operation {id} not found"),
        }
    }
}

impl std::error::Error for JournalError {}

/// An operation to record as `planned`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOp {
    pub op_type: OpType,
    pub source: PathBuf,
    /// `None` for `trash`.
    pub dest: Option<PathBuf>,
    pub size: Option<u64>,
    /// For undo operations: the operation being reverted.
    pub undo_of: Option<i64>,
}

/// Fields written together with a state change. `None` leaves the column as is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateUpdate {
    pub hash_before: Option<String>,
    pub hash_after: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpRecord {
    pub id: i64,
    pub batch_id: String,
    pub seq: i64,
    pub op_type: OpType,
    pub source_path: PathBuf,
    pub dest_path: Option<PathBuf>,
    pub size: Option<u64>,
    pub hash_before: Option<String>,
    pub hash_after: Option<String>,
    pub state: OpState,
    pub error: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub undo_of: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateCounts {
    pub planned: u64,
    pub in_progress: u64,
    pub done: u64,
    pub failed: u64,
    pub rolled_back: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchInfo {
    pub id: String,
    pub kind: BatchKind,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub summary: Option<String>,
    pub counts: StateCounts,
}

/// Handle to the journal tables. Cheap to clone (shares the pool).
#[derive(Debug, Clone)]
pub struct Journal {
    pool: SqlitePool,
}

fn path_text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn op_from_row(r: &SqliteRow) -> Result<OpRecord> {
    Ok(OpRecord {
        id: r.try_get("id")?,
        batch_id: r.try_get("batch_id")?,
        seq: r.try_get("seq")?,
        op_type: r.try_get::<String, _>("op_type")?.parse()?,
        source_path: PathBuf::from(r.try_get::<String, _>("source_path")?),
        dest_path: r
            .try_get::<Option<String>, _>("dest_path")?
            .map(PathBuf::from),
        size: r.try_get::<Option<i64>, _>("size")?.map(|n| n as u64),
        hash_before: r.try_get("hash_before")?,
        hash_after: r.try_get("hash_after")?,
        state: r.try_get::<String, _>("state")?.parse()?,
        error: r.try_get("error")?,
        created_at: r.try_get("created_at")?,
        finished_at: r.try_get("finished_at")?,
        undo_of: r.try_get("undo_of")?,
    })
}

const OP_COLUMNS: &str = "id, batch_id, seq, op_type, source_path, dest_path, size, hash_before, \
                          hash_after, state, error, created_at, finished_at, undo_of";

impl Journal {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Creates an open batch and returns its id (UUID v4).
    pub async fn create_batch(&self, kind: BatchKind) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO batches (id, kind) VALUES (?1, ?2)")
            .bind(&id)
            .bind(kind.as_str())
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    /// Sets `finished_at` and the human-readable summary of a batch.
    pub async fn finish_batch(&self, batch_id: &str, summary: &str) -> Result<()> {
        let done = sqlx::query(&format!(
            "UPDATE batches SET finished_at = {NOW}, summary = ?1 WHERE id = ?2"
        ))
        .bind(summary)
        .bind(batch_id)
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(anyhow!("batch {batch_id} not found"));
        }
        Ok(())
    }

    /// Batches without `finished_at`, oldest first.
    pub async fn unfinished_batches(&self) -> Result<Vec<String>> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM batches WHERE finished_at IS NULL ORDER BY created_at, rowid",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Records `op` as `planned` (one transaction) and returns its id.
    pub async fn insert_planned(&self, batch_id: &str, seq: i64, op: &NewOp) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO operations (batch_id, seq, op_type, source_path, dest_path, size, state, undo_of)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'planned', ?7)",
        )
        .bind(batch_id)
        .bind(seq)
        .bind(op.op_type.as_str())
        .bind(path_text(&op.source))
        .bind(op.dest.as_deref().map(path_text))
        .bind(op.size.map(|n| n as i64))
        .bind(op.undo_of)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// Changes the destination of an operation that has not finished
    /// (collision retry at execution time).
    pub async fn set_dest(&self, op_id: i64, dest: &Path) -> Result<()> {
        let done = sqlx::query(
            "UPDATE operations SET dest_path = ?1 WHERE id = ?2 AND state IN ('planned', 'in_progress')",
        )
        .bind(path_text(dest))
        .bind(op_id)
        .execute(&self.pool)
        .await?;
        if done.rows_affected() == 0 {
            return Err(self.why_not(op_id, OpState::InProgress).await);
        }
        Ok(())
    }

    /// Moves `op_id` to `to`, writing the given fields in the same statement.
    /// Fails with [`JournalError::InvalidTransition`] (state unchanged) when the
    /// state machine does not allow it.
    pub async fn transition(&self, op_id: i64, to: OpState, update: &StateUpdate) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let affected = Self::transition_in(&mut tx, op_id, to, update).await?;
        if affected == 0 {
            drop(tx);
            return Err(self.why_not(op_id, to).await);
        }
        tx.commit().await?;
        Ok(())
    }

    /// `planned -> in_progress`, recording the source hash.
    pub async fn start(&self, op_id: i64, hash_before: Option<&str>) -> Result<()> {
        self.transition(
            op_id,
            OpState::InProgress,
            &StateUpdate {
                hash_before: hash_before.map(str::to_owned),
                ..StateUpdate::default()
            },
        )
        .await
    }

    /// `in_progress -> done`, recording the destination hash.
    pub async fn mark_done(&self, op_id: i64, hash_after: Option<&str>) -> Result<()> {
        self.transition(
            op_id,
            OpState::Done,
            &StateUpdate {
                hash_after: hash_after.map(str::to_owned),
                ..StateUpdate::default()
            },
        )
        .await
    }

    /// `planned | in_progress -> failed` with the error text.
    pub async fn mark_failed(&self, op_id: i64, error: &str) -> Result<()> {
        self.transition(
            op_id,
            OpState::Failed,
            &StateUpdate {
                error: Some(error.to_owned()),
                ..StateUpdate::default()
            },
        )
        .await
    }

    /// Marks an undo operation `done` and the operation it reverts
    /// `rolled_back`, in one transaction.
    pub async fn mark_undo_done(
        &self,
        undo_op_id: i64,
        original_id: i64,
        hash_after: Option<&str>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let update = StateUpdate {
            hash_after: hash_after.map(str::to_owned),
            ..StateUpdate::default()
        };
        if Self::transition_in(&mut tx, undo_op_id, OpState::Done, &update).await? == 0 {
            drop(tx);
            return Err(self.why_not(undo_op_id, OpState::Done).await);
        }
        let none = StateUpdate::default();
        if Self::transition_in(&mut tx, original_id, OpState::RolledBack, &none).await? == 0 {
            drop(tx);
            return Err(self.why_not(original_id, OpState::RolledBack).await);
        }
        tx.commit().await?;
        Ok(())
    }

    async fn transition_in(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        op_id: i64,
        to: OpState,
        update: &StateUpdate,
    ) -> Result<u64> {
        let from: Vec<String> = OpState::ALL
            .iter()
            .filter(|s| s.can_transition_to(to))
            .map(|s| format!("'{}'", s.as_str()))
            .collect();
        if from.is_empty() {
            return Ok(0);
        }
        let finished = if to.is_final_outcome() {
            NOW
        } else {
            "finished_at"
        };
        let sql = format!(
            "UPDATE operations SET state = ?1,
               hash_before = COALESCE(?2, hash_before),
               hash_after = COALESCE(?3, hash_after),
               error = COALESCE(?4, error),
               finished_at = {finished}
             WHERE id = ?5 AND state IN ({})",
            from.join(", ")
        );
        let done = sqlx::query(&sql)
            .bind(to.as_str())
            .bind(&update.hash_before)
            .bind(&update.hash_after)
            .bind(&update.error)
            .bind(op_id)
            .execute(&mut **tx)
            .await?;
        Ok(done.rows_affected())
    }

    /// Explains why an update of `op_id` towards `to` matched no row.
    async fn why_not(&self, op_id: i64, to: OpState) -> anyhow::Error {
        match self.get_op(op_id).await {
            Ok(Some(op)) => JournalError::InvalidTransition {
                op_id,
                from: op.state,
                to,
            }
            .into(),
            Ok(None) => JournalError::OpNotFound(op_id).into(),
            Err(e) => e,
        }
    }

    pub async fn get_op(&self, op_id: i64) -> Result<Option<OpRecord>> {
        let row = sqlx::query(&format!(
            "SELECT {OP_COLUMNS} FROM operations WHERE id = ?1"
        ))
        .bind(op_id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(op_from_row).transpose()
    }

    /// Operations of a batch in `seq` order.
    pub async fn list_ops(&self, batch_id: &str) -> Result<Vec<OpRecord>> {
        let rows = sqlx::query(&format!(
            "SELECT {OP_COLUMNS} FROM operations WHERE batch_id = ?1 ORDER BY seq, id"
        ))
        .bind(batch_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(op_from_row).collect()
    }

    /// Operations left `planned` or `in_progress` (by a crash), oldest first.
    pub async fn unfinished_ops(&self) -> Result<Vec<OpRecord>> {
        let rows = sqlx::query(&format!(
            "SELECT {OP_COLUMNS} FROM operations
             WHERE state IN ('planned', 'in_progress') ORDER BY id"
        ))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(op_from_row).collect()
    }

    /// Newest batches first, with the number of operations in each state.
    pub async fn list_batches(&self, limit: u32) -> Result<Vec<BatchInfo>> {
        self.batches_where("1 = 1", None, limit).await
    }

    pub async fn get_batch(&self, batch_id: &str) -> Result<Option<BatchInfo>> {
        Ok(self
            .batches_where("b.id = ?2", Some(batch_id), 1)
            .await?
            .into_iter()
            .next())
    }

    async fn batches_where(
        &self,
        filter: &str,
        id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<BatchInfo>> {
        let sql = format!(
            "SELECT b.id, b.kind, b.created_at, b.finished_at, b.summary,
               COALESCE(SUM(o.state = 'planned'), 0) AS n_planned,
               COALESCE(SUM(o.state = 'in_progress'), 0) AS n_in_progress,
               COALESCE(SUM(o.state = 'done'), 0) AS n_done,
               COALESCE(SUM(o.state = 'failed'), 0) AS n_failed,
               COALESCE(SUM(o.state = 'rolled_back'), 0) AS n_rolled_back
             FROM batches b LEFT JOIN operations o ON o.batch_id = b.id
             WHERE {filter}
             GROUP BY b.id
             ORDER BY b.created_at DESC, b.rowid DESC
             LIMIT ?1"
        );
        let mut q = sqlx::query(&sql).bind(i64::from(limit));
        if let Some(id) = id {
            q = q.bind(id);
        }
        let rows = q.fetch_all(&self.pool).await?;
        rows.iter()
            .map(|r| {
                let n = |c: &str| r.try_get::<i64, _>(c).map(|v| v as u64);
                Ok(BatchInfo {
                    id: r.try_get("id")?,
                    kind: r.try_get::<String, _>("kind")?.parse()?,
                    created_at: r.try_get("created_at")?,
                    finished_at: r.try_get("finished_at")?,
                    summary: r.try_get("summary")?,
                    counts: StateCounts {
                        planned: n("n_planned")?,
                        in_progress: n("n_in_progress")?,
                        done: n("n_done")?,
                        failed: n("n_failed")?,
                        rolled_back: n("n_rolled_back")?,
                    },
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Db;
    use OpState::*;

    async fn journal() -> (tempfile::TempDir, Db, Journal) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("j.sqlite3")).await.unwrap();
        let j = db.journal();
        (tmp, db, j)
    }

    fn mv(src: &str, dest: &str) -> NewOp {
        NewOp {
            op_type: OpType::Move,
            source: PathBuf::from(src),
            dest: Some(PathBuf::from(dest)),
            size: Some(3),
            undo_of: None,
        }
    }

    fn invalid(err: &anyhow::Error) -> Option<&JournalError> {
        err.downcast_ref::<JournalError>()
    }

    #[test]
    fn state_machine_table() {
        let allowed = [
            (Planned, InProgress),
            (Planned, Failed),
            (InProgress, Done),
            (InProgress, Failed),
            (Done, RolledBack),
        ];
        for from in OpState::ALL {
            for to in OpState::ALL {
                assert_eq!(
                    from.can_transition_to(to),
                    allowed.contains(&(from, to)),
                    "{from} -> {to}"
                );
            }
        }
    }

    #[test]
    fn enums_round_trip_through_text() {
        for s in OpState::ALL {
            assert_eq!(s.as_str().parse::<OpState>().unwrap(), s);
        }
        for k in [BatchKind::Apply, BatchKind::Undo, BatchKind::Recovery] {
            assert_eq!(k.as_str().parse::<BatchKind>().unwrap(), k);
        }
        for t in [OpType::Move, OpType::Copy, OpType::Trash, OpType::Restore] {
            assert_eq!(t.as_str().parse::<OpType>().unwrap(), t);
        }
        assert!("bogus".parse::<OpState>().is_err());
        assert_eq!(
            serde_json::to_string(&InProgress).unwrap(),
            "\"in_progress\""
        );
    }

    /// A database written before migration 0004 keeps its operations (ids,
    /// undo links, the AUTOINCREMENT counter) and then accepts `restore`.
    #[tokio::test]
    async fn migration_0004_keeps_rows_and_allows_restore() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("old.sqlite3");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let mut old = sqlx::migrate!("./migrations");
        old.migrations = std::borrow::Cow::Owned(
            old.migrations
                .iter()
                .filter(|m| m.version < 4)
                .cloned()
                .collect(),
        );
        old.run(&pool).await.unwrap();
        let j = Journal::new(pool.clone());
        let b = j.create_batch(BatchKind::Apply).await.unwrap();
        let first = j.insert_planned(&b, 0, &mv("a", "b")).await.unwrap();
        j.start(first, Some("h")).await.unwrap();
        j.mark_done(first, Some("h")).await.unwrap();
        let mut undo = mv("b", "a");
        undo.undo_of = Some(first);
        let second = j.insert_planned(&b, 1, &undo).await.unwrap();
        // A row deleted at the end leaves the counter above the max id.
        let third = j.insert_planned(&b, 2, &mv("c", "d")).await.unwrap();
        sqlx::query("DELETE FROM operations WHERE id = ?1")
            .bind(third)
            .execute(&pool)
            .await
            .unwrap();
        let restore = NewOp {
            op_type: OpType::Restore,
            source: PathBuf::from("a"),
            dest: Some(PathBuf::from("a")),
            size: None,
            undo_of: Some(first),
        };
        assert!(
            j.insert_planned(&b, 3, &restore).await.is_err(),
            "v3 refuses restore"
        );
        pool.close().await;

        let db = Db::connect(&path).await.unwrap();
        let j = db.journal();
        let ops = j.list_ops(&b).await.unwrap();
        assert_eq!(
            ops.iter().map(|o| o.id).collect::<Vec<_>>(),
            [first, second]
        );
        assert_eq!(ops[0].state, Done);
        assert_eq!(ops[0].hash_after.as_deref(), Some("h"));
        assert_eq!(ops[1].undo_of, Some(first));
        let id = j.insert_planned(&b, 3, &restore).await.unwrap();
        assert!(id > third, "ids are not reused");
        assert_eq!(
            j.get_op(id).await.unwrap().unwrap().op_type,
            OpType::Restore
        );
        let broken: Vec<(String,)> =
            sqlx::query_as("SELECT \"table\" FROM pragma_foreign_key_check")
                .fetch_all(&db.pool)
                .await
                .unwrap();
        assert!(broken.is_empty(), "{broken:?}");
        let indexes: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'operations' AND name LIKE 'idx_%' ORDER BY name",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert_eq!(indexes, ["idx_operations_batch", "idx_operations_state"]);
    }

    #[tokio::test]
    async fn connect_enables_wal() {
        let (_tmp, db, _j) = journal().await;
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[tokio::test]
    async fn happy_path_records_fields_per_state() {
        let (_tmp, _db, j) = journal().await;
        let b = j.create_batch(BatchKind::Apply).await.unwrap();
        let id = j
            .insert_planned(&b, 0, &mv("a/x.txt", "b/x.txt"))
            .await
            .unwrap();
        let op = j.get_op(id).await.unwrap().unwrap();
        assert_eq!(op.state, Planned);
        assert_eq!(op.dest_path, Some(PathBuf::from("b/x.txt")));
        assert_eq!(op.size, Some(3));
        assert!(op.finished_at.is_none());

        j.start(id, Some("h1")).await.unwrap();
        j.set_dest(id, Path::new("b/x (2).txt")).await.unwrap();
        j.mark_done(id, Some("h1")).await.unwrap();
        let op = j.get_op(id).await.unwrap().unwrap();
        assert_eq!(op.state, Done);
        assert_eq!(op.hash_before.as_deref(), Some("h1"));
        assert_eq!(op.hash_after.as_deref(), Some("h1"));
        assert_eq!(op.dest_path, Some(PathBuf::from("b/x (2).txt")));
        assert!(op.finished_at.is_some());

        j.transition(id, RolledBack, &StateUpdate::default())
            .await
            .unwrap();
        assert_eq!(j.get_op(id).await.unwrap().unwrap().state, RolledBack);
    }

    #[tokio::test]
    async fn every_disallowed_transition_is_rejected_and_state_kept() {
        let (_tmp, _db, j) = journal().await;
        let b = j.create_batch(BatchKind::Apply).await.unwrap();
        // Build one op in each state, then try every disallowed target.
        let mut seq = 0;
        for from in OpState::ALL {
            for to in OpState::ALL
                .into_iter()
                .filter(|t| !from.can_transition_to(*t))
            {
                seq += 1;
                let id = j.insert_planned(&b, seq, &mv("s", "d")).await.unwrap();
                let path: &[OpState] = match from {
                    Planned => &[],
                    InProgress => &[InProgress],
                    Done => &[InProgress, Done],
                    Failed => &[Failed],
                    RolledBack => &[InProgress, Done, RolledBack],
                };
                for step in path {
                    j.transition(id, *step, &StateUpdate::default())
                        .await
                        .unwrap();
                }
                let err = j
                    .transition(id, to, &StateUpdate::default())
                    .await
                    .unwrap_err();
                assert_eq!(
                    invalid(&err),
                    Some(&JournalError::InvalidTransition {
                        op_id: id,
                        from,
                        to
                    }),
                    "{from} -> {to}"
                );
                assert_eq!(j.get_op(id).await.unwrap().unwrap().state, from);
            }
        }
    }

    #[tokio::test]
    async fn missing_op_is_reported() {
        let (_tmp, _db, j) = journal().await;
        let err = j.start(999, None).await.unwrap_err();
        assert_eq!(invalid(&err), Some(&JournalError::OpNotFound(999)));
    }

    #[tokio::test]
    async fn set_dest_rejected_once_finished() {
        let (_tmp, _db, j) = journal().await;
        let b = j.create_batch(BatchKind::Apply).await.unwrap();
        let id = j.insert_planned(&b, 0, &mv("s", "d")).await.unwrap();
        j.mark_failed(id, "SourceMissing").await.unwrap();
        assert!(j.set_dest(id, Path::new("other")).await.is_err());
        let op = j.get_op(id).await.unwrap().unwrap();
        assert_eq!(op.dest_path, Some(PathBuf::from("d")));
        assert_eq!(op.error.as_deref(), Some("SourceMissing"));
        assert!(op.finished_at.is_some());
    }

    #[tokio::test]
    async fn undo_done_and_rollback_are_atomic() {
        let (_tmp, _db, j) = journal().await;
        let b = j.create_batch(BatchKind::Apply).await.unwrap();
        let orig = j.insert_planned(&b, 0, &mv("s", "d")).await.unwrap();
        j.start(orig, Some("h")).await.unwrap();
        j.mark_done(orig, Some("h")).await.unwrap();

        let u = j.create_batch(BatchKind::Undo).await.unwrap();
        let mut back = mv("d", "s");
        back.undo_of = Some(orig);
        let undo = j.insert_planned(&u, 0, &back).await.unwrap();
        // Not started: the pair must fail as a whole.
        assert!(j.mark_undo_done(undo, orig, Some("h")).await.is_err());
        assert_eq!(j.get_op(orig).await.unwrap().unwrap().state, Done);

        j.start(undo, Some("h")).await.unwrap();
        j.mark_undo_done(undo, orig, Some("h")).await.unwrap();
        assert_eq!(j.get_op(orig).await.unwrap().unwrap().state, RolledBack);
        let undo_rec = j.get_op(undo).await.unwrap().unwrap();
        assert_eq!(undo_rec.state, Done);
        assert_eq!(undo_rec.undo_of, Some(orig));
    }

    #[tokio::test]
    async fn batches_newest_first_with_counts() {
        let (_tmp, _db, j) = journal().await;
        let first = j.create_batch(BatchKind::Apply).await.unwrap();
        let a = j.insert_planned(&first, 0, &mv("a", "A")).await.unwrap();
        let b = j.insert_planned(&first, 1, &mv("b", "B")).await.unwrap();
        let _c = j.insert_planned(&first, 2, &mv("c", "C")).await.unwrap();
        j.start(a, None).await.unwrap();
        j.mark_done(a, None).await.unwrap();
        j.mark_failed(b, "Locked").await.unwrap();
        j.finish_batch(&first, "1 done, 1 failed").await.unwrap();
        let second = j.create_batch(BatchKind::Undo).await.unwrap();

        let list = j.list_batches(10).await.unwrap();
        assert_eq!(
            list.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
            vec![second.as_str(), first.as_str()]
        );
        assert_eq!(list[0].kind, BatchKind::Undo);
        assert_eq!(list[0].counts, StateCounts::default());
        assert!(list[0].finished_at.is_none());
        assert_eq!(
            list[1].counts,
            StateCounts {
                planned: 1,
                done: 1,
                failed: 1,
                ..StateCounts::default()
            }
        );
        assert_eq!(list[1].summary.as_deref(), Some("1 done, 1 failed"));
        assert_eq!(j.list_batches(1).await.unwrap().len(), 1);
        assert_eq!(j.get_batch(&first).await.unwrap().unwrap(), list[1]);
        assert!(j.get_batch("nope").await.unwrap().is_none());
        assert_eq!(j.unfinished_batches().await.unwrap(), vec![second.clone()]);
        assert!(j.finish_batch("nope", "x").await.is_err());
    }

    #[tokio::test]
    async fn ops_listed_by_seq_and_unfinished_found() {
        let (_tmp, _db, j) = journal().await;
        let b = j.create_batch(BatchKind::Apply).await.unwrap();
        let late = j.insert_planned(&b, 5, &mv("late", "L")).await.unwrap();
        let early = j.insert_planned(&b, 1, &mv("early", "E")).await.unwrap();
        let mid = j.insert_planned(&b, 3, &mv("mid", "M")).await.unwrap();
        j.start(mid, None).await.unwrap();
        j.start(early, None).await.unwrap();
        j.mark_done(early, None).await.unwrap();

        let seqs: Vec<i64> = j
            .list_ops(&b)
            .await
            .unwrap()
            .iter()
            .map(|o| o.seq)
            .collect();
        assert_eq!(seqs, vec![1, 3, 5]);
        let unfinished: Vec<(i64, OpState)> = j
            .unfinished_ops()
            .await
            .unwrap()
            .iter()
            .map(|o| (o.id, o.state))
            .collect();
        assert_eq!(unfinished, vec![(late, Planned), (mid, InProgress)]);
    }
}
