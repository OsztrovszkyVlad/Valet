//! Runs planned operations under the write-ahead journal (PLAN.md 4.2, 4.3).
//!
//! Per operation: journal `planned` -> re-check the source -> hash it ->
//! `in_progress` -> act -> verify -> `done` or `failed`.
//!
//! - Same-volume move: `Platform::rename_no_replace`.
//! - Cross-volume move: copy into `.valet-tmp-<op id>` in the destination folder,
//!   fsync, compare blake3 with the source hash, rename into place, then send
//!   the source to the Recycle Bin. Requires the Recycle Bin on both sides.
//! - Copy: as cross-volume move, without trashing the source.
//!
//! Nothing is deleted permanently and nothing is overwritten. Destination
//! folders created on the way are left in place (also by undo).
//!
//! The functions are `async` because the journal is; file I/O runs inline.
//!
//! Test hook, compiled only with the `test-support` feature: when the
//! environment variable `VALET_FAULT_SLOW_MS` holds a number, the executor
//! sleeps that many milliseconds after every journal state change, so the
//! fault-injection harness can kill the process mid-batch. In normal builds
//! the variable has no effect.

use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    collision::next_free_name,
    journal::{BatchKind, Journal, NewOp, OpType},
    planner::{OpKind, PlannedOp},
    platform::Platform,
};

mod recover;
pub use recover::{RecoveryDecision, RecoveryReport, recover};
mod undo;
pub use undo::{
    ConfirmReason, NeedsConfirmation, Restored, SkippedOp, UndoFailure, UndoReport, undo_batch,
};

/// Environment variable read by the fault-injection test hook (milliseconds).
#[cfg(feature = "test-support")]
pub const FAULT_SLOW_ENV: &str = "VALET_FAULT_SLOW_MS";

/// Attempts to find a free destination name when the planned one got taken.
const MAX_COLLISION_RETRIES: u32 = 100;

/// Why an operation did not complete. Stored in the journal's `error` column
/// as `Code` or `Code: detail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailCode {
    /// The source file is gone.
    SourceMissing,
    /// Size, mtime or type of the source differ from the preview.
    ChangedSincePreview,
    /// Unhydrated cloud placeholder; never opened or hashed.
    CloudPlaceholder,
    /// Another process holds the file.
    Locked,
    /// The operation would need the Recycle Bin where it is not available.
    NoRecycleBin,
    /// The copied bytes or the result on disk do not match.
    VerifyFailed,
    /// The source could not be sent to the Recycle Bin; the copy was removed again.
    TrashFailed,
    /// The target path is taken and renaming was not allowed.
    Occupied,
    /// Stopped by a crash; decided by recovery.
    Interrupted,
    /// Path cannot be stored exactly (not valid Unicode) or has no parent folder.
    InvalidPath,
    /// Any other I/O error.
    Io,
}

impl FailCode {
    pub fn as_str(self) -> &'static str {
        match self {
            FailCode::SourceMissing => "SourceMissing",
            FailCode::ChangedSincePreview => "ChangedSincePreview",
            FailCode::CloudPlaceholder => "CloudPlaceholder",
            FailCode::Locked => "Locked",
            FailCode::NoRecycleBin => "NoRecycleBin",
            FailCode::VerifyFailed => "VerifyFailed",
            FailCode::TrashFailed => "TrashFailed",
            FailCode::Occupied => "Occupied",
            FailCode::Interrupted => "Interrupted",
            FailCode::InvalidPath => "InvalidPath",
            FailCode::Io => "Io",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpError {
    pub code: FailCode,
    pub detail: Option<String>,
}

impl OpError {
    fn new(code: FailCode) -> Self {
        Self { code, detail: None }
    }

    fn with(code: FailCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }

    fn io(e: &io::Error) -> Self {
        Self::with(FailCode::Io, e.to_string())
    }

    /// Text for the journal's `error` column.
    pub fn journal_text(&self) -> String {
        match &self.detail {
            Some(d) => format!("{}: {d}", self.code.as_str()),
            None => self.code.as_str().to_owned(),
        }
    }
}

/// Outcome of one planned operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpResult {
    pub op_id: i64,
    pub kind: OpKind,
    pub source: PathBuf,
    /// Final destination (differs from the plan after a collision retry).
    pub dest: PathBuf,
    pub rule_name: String,
    pub size: u64,
    /// `None` when the operation is `done`.
    pub error: Option<OpError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchReport {
    pub batch_id: String,
    /// In plan order.
    pub results: Vec<OpResult>,
}

impl BatchReport {
    pub fn done_count(&self) -> usize {
        self.results.iter().filter(|r| r.error.is_none()).count()
    }

    pub fn failed_count(&self) -> usize {
        self.results.len() - self.done_count()
    }
}

// ---------------------------------------------------------------- helpers

/// Fault-injection test hook, see the module docs.
#[cfg(feature = "test-support")]
fn fault_pause() {
    if let Some(ms) = std::env::var(FAULT_SLOW_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|ms| *ms > 0)
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// Shipped builds: no hook. There is no `cfg(not(feature = "test-support"))`
/// unit test for this, because the crate's self dev-dependency turns
/// `test-support` on for every `cargo test -p valet-core` build, so such a
/// test would never compile or run. Checked instead by building the desktop
/// binary alone and confirming `VALET_FAULT_SLOW_MS` does not occur in it.
#[cfg(not(feature = "test-support"))]
#[inline(always)]
fn fault_pause() {}

/// Journal calls that change an operation's state, each followed by the test hook.
struct Steps<'a> {
    journal: &'a Journal,
}

impl Steps<'_> {
    async fn planned(&self, batch_id: &str, seq: i64, op: &NewOp) -> Result<i64> {
        let id = self.journal.insert_planned(batch_id, seq, op).await?;
        fault_pause();
        Ok(id)
    }

    async fn start(&self, op_id: i64, hash_before: Option<&str>) -> Result<()> {
        self.journal.start(op_id, hash_before).await?;
        fault_pause();
        Ok(())
    }

    async fn done(&self, op_id: i64, hash_after: Option<&str>) -> Result<()> {
        self.journal.mark_done(op_id, hash_after).await?;
        fault_pause();
        Ok(())
    }

    async fn undo_done(&self, op_id: i64, original: i64, hash_after: Option<&str>) -> Result<()> {
        self.journal
            .mark_undo_done(op_id, original, hash_after)
            .await?;
        fault_pause();
        Ok(())
    }

    async fn failed(&self, op_id: i64, err: &OpError) -> Result<()> {
        self.journal.mark_failed(op_id, &err.journal_text()).await?;
        fault_pause();
        Ok(())
    }
}

/// Streaming blake3 of a file, as lowercase hex.
pub(crate) fn hash_file(platform: &dyn Platform, path: &Path) -> io::Result<String> {
    let mut f = File::open(platform.os_path(path))?;
    let mut h = blake3::Hasher::new();
    h.update_reader(&mut f)?;
    Ok(h.finalize().to_hex().to_string())
}

fn exists(platform: &dyn Platform, path: &Path) -> bool {
    fs::symlink_metadata(platform.os_path(path)).is_ok()
}

/// Name of the copy-in-progress file for operation `op_id` in `dir`.
pub(crate) fn temp_path(dir: &Path, op_id: i64) -> PathBuf {
    dir.join(format!(".valet-tmp-{op_id}"))
}

fn op_type(kind: OpKind) -> OpType {
    match kind {
        OpKind::Move => OpType::Move,
        OpKind::Copy => OpType::Copy,
    }
}

/// What to do when the target name is taken at execution time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnCollision {
    /// Pick the next free `name (n).ext` and record it in the journal.
    NextFreeName,
    /// Fail with [`FailCode::Occupied`].
    Fail,
}

/// One file transfer of a journaled operation that is `in_progress`.
struct Transfer<'a> {
    op_id: i64,
    source: &'a Path,
    dest: PathBuf,
    /// blake3 of the source taken before `in_progress`.
    hash: &'a str,
    size: u64,
    on_collision: OnCollision,
}

/// Renames `from` to the transfer's destination without replacing anything.
/// Returns the final destination.
async fn place(
    from: &Path,
    t: &Transfer<'_>,
    journal: &Journal,
    platform: &dyn Platform,
) -> Result<Result<PathBuf, OpError>> {
    let mut dest = t.dest.clone();
    for _ in 0..MAX_COLLISION_RETRIES {
        match platform.rename_no_replace(from, &dest) {
            Ok(()) => return Ok(Ok(dest)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if t.on_collision == OnCollision::Fail {
                    return Ok(Err(OpError::with(
                        FailCode::Occupied,
                        format!("{} exists", dest.display()),
                    )));
                }
                dest = next_free_name(&t.dest, |p| exists(platform, p));
                // The journal names the path we are about to try, so recovery
                // looks in the right place.
                journal.set_dest(t.op_id, &dest).await?;
            }
            Err(e) => return Ok(Err(OpError::io(&e))),
        }
    }
    Ok(Err(OpError::with(
        FailCode::Io,
        format!("no free destination name after {MAX_COLLISION_RETRIES} attempts"),
    )))
}

/// The file at `dest` has the expected size (and the source is gone, for moves).
fn verify_placed(
    platform: &dyn Platform,
    source: &Path,
    dest: &Path,
    size: u64,
    source_gone: bool,
) -> Result<(), OpError> {
    match fs::symlink_metadata(platform.os_path(dest)) {
        Ok(m) if m.is_file() && m.len() == size => {}
        Ok(m) => {
            return Err(OpError::with(
                FailCode::VerifyFailed,
                format!("{} has {} bytes, expected {size}", dest.display(), m.len()),
            ));
        }
        Err(e) => {
            return Err(OpError::with(
                FailCode::VerifyFailed,
                format!("{} not found after the operation: {e}", dest.display()),
            ));
        }
    }
    if source_gone && exists(platform, source) {
        return Err(OpError::with(
            FailCode::VerifyFailed,
            format!(
                "{} still exists; the file is also at {}",
                source.display(),
                dest.display()
            ),
        ));
    }
    Ok(())
}

/// Same-volume move by atomic rename.
async fn rename_move(
    t: &Transfer<'_>,
    journal: &Journal,
    platform: &dyn Platform,
) -> Result<Result<PathBuf, OpError>> {
    let dest = match place(t.source, t, journal, platform).await? {
        Ok(d) => d,
        Err(e) => return Ok(Err(e)),
    };
    Ok(verify_placed(platform, t.source, &dest, t.size, true).map(|()| dest))
}

/// Sends a file Valet created (temp or copy) to the Recycle Bin; on failure
/// returns a note for the error detail.
fn trash_own(platform: &dyn Platform, path: &Path) -> Option<String> {
    platform.trash(path).err().map(|e| {
        format!(
            "; {} could not be sent to the Recycle Bin: {e}",
            path.display()
        )
    })
}

/// Writes the source into a fresh temp file in the destination folder, fsyncs
/// it and returns its blake3.
fn copy_to_temp(platform: &dyn Platform, source: &Path, temp: &Path) -> (bool, io::Result<String>) {
    let mut out = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(platform.os_path(temp))
    {
        Ok(f) => f,
        Err(e) => return (false, Err(e)),
    };
    let res = (|| {
        let mut src = File::open(platform.os_path(source))?;
        io::copy(&mut src, &mut out)?;
        out.sync_all()?;
        Ok(())
    })();
    drop(out);
    (true, res.and_then(|()| hash_file(platform, temp)))
}

/// Copy + verify + rename into place; with `trash_source`, then sends the
/// source to the Recycle Bin (cross-volume move).
async fn copy_transfer(
    t: &Transfer<'_>,
    trash_source: bool,
    journal: &Journal,
    platform: &dyn Platform,
) -> Result<Result<PathBuf, OpError>> {
    let Some(dir) = t.dest.parent() else {
        return Ok(Err(OpError::new(FailCode::InvalidPath)));
    };
    let temp = temp_path(dir, t.op_id);
    let (created, copied) = copy_to_temp(platform, t.source, &temp);
    let copied_hash = match copied {
        Ok(h) => h,
        Err(e) => {
            let mut err = OpError::io(&e);
            if created {
                err.detail = Some(format!(
                    "{}{}",
                    e,
                    trash_own(platform, &temp).unwrap_or_default()
                ));
            }
            return Ok(Err(err));
        }
    };
    if copied_hash != t.hash {
        return Ok(Err(OpError::with(
            FailCode::VerifyFailed,
            format!(
                "copy differs from the source{}",
                trash_own(platform, &temp).unwrap_or_default()
            ),
        )));
    }
    let dest = match place(&temp, t, journal, platform).await? {
        Ok(d) => d,
        Err(mut e) => {
            if let Some(note) = trash_own(platform, &temp) {
                e.detail = Some(format!("{}{note}", e.detail.unwrap_or_default()));
            }
            return Ok(Err(e));
        }
    };
    if trash_source && let Err(e) = platform.trash(t.source) {
        // Keep the file exactly once: at the source.
        let note = trash_own(platform, &dest).unwrap_or_default();
        return Ok(Err(OpError::with(
            FailCode::TrashFailed,
            format!("source kept, copy removed: {e}{note}"),
        )));
    }
    Ok(verify_placed(platform, t.source, &dest, t.size, trash_source).map(|()| dest))
}

/// Checks done right before acting, in this order: placeholder (before
/// anything touches the file), existence, type, size, mtime, lock.
fn precheck(op: &PlannedOp, platform: &dyn Platform) -> Result<(), OpError> {
    if op.source.to_str().is_none() || op.dest.to_str().is_none() {
        return Err(OpError::with(
            FailCode::InvalidPath,
            "path is not valid Unicode",
        ));
    }
    if op.dest.parent().is_none_or(|p| p.as_os_str().is_empty()) {
        return Err(OpError::with(
            FailCode::InvalidPath,
            "destination has no folder",
        ));
    }
    if platform.is_cloud_placeholder(&op.source) {
        return Err(OpError::new(FailCode::CloudPlaceholder));
    }
    let meta = match fs::symlink_metadata(platform.os_path(&op.source)) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(OpError::new(FailCode::SourceMissing));
        }
        Err(e) => return Err(OpError::io(&e)),
    };
    if !meta.is_file() || platform.is_reparse_point(&op.source) {
        return Err(OpError::with(
            FailCode::ChangedSincePreview,
            "no longer a regular file",
        ));
    }
    if meta.len() != op.size {
        return Err(OpError::with(
            FailCode::ChangedSincePreview,
            format!("size {} -> {}", op.size, meta.len()),
        ));
    }
    if meta.modified().ok() != Some(op.mtime) {
        return Err(OpError::with(
            FailCode::ChangedSincePreview,
            "modification time changed",
        ));
    }
    if platform.is_locked(&op.source) {
        return Err(OpError::new(FailCode::Locked));
    }
    Ok(())
}

// ---------------------------------------------------------------- execute

/// Executes `ops` in order as one `apply` batch. Each operation is journaled
/// before it runs; a failed operation does not stop the batch. Returns `Err`
/// only when the journal cannot be written (recovery handles what is left).
pub async fn execute(
    ops: &[PlannedOp],
    journal: &Journal,
    platform: &dyn Platform,
) -> Result<BatchReport> {
    let steps = Steps { journal };
    let batch_id = journal.create_batch(BatchKind::Apply).await?;
    let mut results = Vec::with_capacity(ops.len());
    for (seq, op) in ops.iter().enumerate() {
        let op_id = steps
            .planned(
                &batch_id,
                seq as i64,
                &NewOp {
                    op_type: op_type(op.kind),
                    source: op.source.clone(),
                    dest: Some(op.dest.clone()),
                    size: Some(op.size),
                    undo_of: None,
                },
            )
            .await?;
        let outcome = execute_one(op_id, op, &steps, platform).await?;
        let (dest, error) = match outcome {
            Ok(dest) => (dest, None),
            Err(e) => (
                journal
                    .get_op(op_id)
                    .await?
                    .and_then(|r| r.dest_path)
                    .unwrap_or_else(|| op.dest.clone()),
                Some(e),
            ),
        };
        results.push(OpResult {
            op_id,
            kind: op.kind,
            source: op.source.clone(),
            dest,
            rule_name: op.rule_name.clone(),
            size: op.size,
            error,
        });
    }
    let report = BatchReport { batch_id, results };
    journal
        .finish_batch(
            &report.batch_id,
            &format!(
                "{} done, {} failed",
                report.done_count(),
                report.failed_count()
            ),
        )
        .await?;
    Ok(report)
}

/// One operation that is already `planned` in the journal.
async fn execute_one(
    op_id: i64,
    op: &PlannedOp,
    steps: &Steps<'_>,
    platform: &dyn Platform,
) -> Result<Result<PathBuf, OpError>> {
    let ready = (|| {
        precheck(op, platform)?;
        let hash = hash_file(platform, &op.source).map_err(|e| OpError::io(&e))?;
        let dir = op.dest.parent().expect("checked by precheck");
        fs::create_dir_all(platform.os_path(dir)).map_err(|e| OpError::io(&e))?;
        let same_volume = matches!(platform.same_volume(&op.source, dir), Ok(true));
        let renames = op.kind == OpKind::Move && same_volume;
        // A cross-volume move trashes the source; any copy may have to trash
        // its temp file, and undoing a copy trashes the copy.
        let needs_trash = match op.kind {
            OpKind::Move => !same_volume,
            OpKind::Copy => true,
        };
        if needs_trash
            && !(platform.can_trash(dir)
                && (op.kind == OpKind::Copy || platform.can_trash(&op.source)))
        {
            return Err(OpError::new(FailCode::NoRecycleBin));
        }
        Ok((hash, renames))
    })();
    let (hash, renames) = match ready {
        Ok(r) => r,
        Err(e) => {
            steps.failed(op_id, &e).await?;
            return Ok(Err(e));
        }
    };

    steps.start(op_id, Some(&hash)).await?;
    let t = Transfer {
        op_id,
        source: &op.source,
        dest: op.dest.clone(),
        hash: &hash,
        size: op.size,
        on_collision: OnCollision::NextFreeName,
    };
    let outcome = if renames {
        rename_move(&t, steps.journal, platform).await?
    } else {
        copy_transfer(&t, op.kind == OpKind::Move, steps.journal, platform).await?
    };
    match &outcome {
        // Rename and verified copy both leave content equal to the source hash.
        Ok(_) => steps.done(op_id, Some(&hash)).await?,
        Err(e) => steps.failed(op_id, e).await?,
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests;
