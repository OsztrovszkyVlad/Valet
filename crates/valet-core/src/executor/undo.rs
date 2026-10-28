//! Undo of an `apply` batch: inverse operations in reverse order, journaled as
//! a new `undo` batch whose operations point at the originals (`undo_of`).
//!
//! Before each step the file at the destination must still hash to the
//! recorded `hash_after`. When it does not, or when the original path is now
//! taken, the operation is left alone and reported as needing confirmation;
//! calling again with `force` undoes it anyway (a taken original path then
//! gets the next free `name (n).ext`, listed in `restored_as`). Nothing is
//! ever overwritten. Folders created by the batch stay.

use anyhow::{anyhow, bail};

use super::*;
use crate::journal::{OpRecord, OpState};

/// Why an operation needs the user's confirmation before it is undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfirmReason {
    /// The file at the destination changed since the batch ran.
    Modified,
    /// Something now occupies the original path of a moved file.
    SourceOccupied,
    /// Undoing a copy, but the original file is gone (the copy is the last one).
    OriginalMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedsConfirmation {
    pub op_id: i64,
    pub source: PathBuf,
    pub dest: Option<PathBuf>,
    pub reasons: Vec<ConfirmReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoFailure {
    pub op_id: i64,
    pub source: PathBuf,
    pub dest: Option<PathBuf>,
    pub error: OpError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedOp {
    pub op_id: i64,
    pub state: OpState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Restored {
    pub op_id: i64,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UndoReport {
    pub batch_id: String,
    /// The new `undo` batch; `None` when no file was touched.
    pub undo_batch_id: Option<String>,
    /// Original operations now `rolled_back`.
    pub undone: Vec<i64>,
    /// Moved files restored under a new name because the original path was taken.
    pub restored_as: Vec<Restored>,
    pub needs_confirmation: Vec<NeedsConfirmation>,
    pub failed: Vec<UndoFailure>,
    /// Operations that were not `done` (failed, already rolled back, ...).
    pub skipped: Vec<SkippedOp>,
}

struct Undoer<'a> {
    steps: Steps<'a>,
    platform: &'a dyn Platform,
    force: bool,
    undo_batch: Option<String>,
    seq: i64,
    report: UndoReport,
}

/// Undoes the `done` operations of the `apply` batch `batch_id`, last first.
pub async fn undo_batch(
    batch_id: &str,
    journal: &Journal,
    platform: &dyn Platform,
    force: bool,
) -> Result<UndoReport> {
    let batch = journal
        .get_batch(batch_id)
        .await?
        .ok_or_else(|| anyhow!("batch {batch_id} not found"))?;
    if batch.kind != BatchKind::Apply {
        bail!(
            "only apply batches can be undone; {batch_id} is {}",
            batch.kind
        );
    }
    if batch.finished_at.is_none() {
        bail!("batch {batch_id} has not finished; run recovery first");
    }
    let mut u = Undoer {
        steps: Steps { journal },
        platform,
        force,
        undo_batch: None,
        seq: 0,
        report: UndoReport {
            batch_id: batch_id.to_owned(),
            ..UndoReport::default()
        },
    };
    let mut ops = journal.list_ops(batch_id).await?;
    ops.reverse();
    for op in &ops {
        if op.state == OpState::Done {
            u.undo_one(op).await?;
        } else {
            u.report.skipped.push(SkippedOp {
                op_id: op.id,
                state: op.state,
            });
        }
    }
    let r = &u.report;
    if let Some(ub) = &u.undo_batch {
        journal
            .finish_batch(
                ub,
                &format!(
                    "undo of batch {batch_id}: {} undone, {} need confirmation, {} failed, {} skipped",
                    r.undone.len(),
                    r.needs_confirmation.len(),
                    r.failed.len(),
                    r.skipped.len()
                ),
            )
            .await?;
    }
    u.report.undo_batch_id = u.undo_batch.clone();
    Ok(u.report)
}

impl Undoer<'_> {
    async fn batch(&mut self) -> Result<String> {
        if self.undo_batch.is_none() {
            self.undo_batch = Some(self.steps.journal.create_batch(BatchKind::Undo).await?);
        }
        Ok(self.undo_batch.clone().expect("set above"))
    }

    fn fail(&mut self, op: &OpRecord, error: OpError) {
        self.report.failed.push(UndoFailure {
            op_id: op.id,
            source: op.source_path.clone(),
            dest: op.dest_path.clone(),
            error,
        });
    }

    fn confirm(&mut self, op: &OpRecord, reasons: Vec<ConfirmReason>) {
        self.report.needs_confirmation.push(NeedsConfirmation {
            op_id: op.id,
            source: op.source_path.clone(),
            dest: op.dest_path.clone(),
            reasons,
        });
    }

    async fn undo_one(&mut self, op: &OpRecord) -> Result<()> {
        let Some(dest) = op.dest_path.clone() else {
            self.fail(
                op,
                OpError::with(FailCode::InvalidPath, "no destination recorded"),
            );
            return Ok(());
        };
        // The file to bring back: never hash a placeholder.
        if self.platform.is_cloud_placeholder(&dest) {
            self.fail(op, OpError::new(FailCode::CloudPlaceholder));
            return Ok(());
        }
        let size = match fs::symlink_metadata(self.platform.os_path(&dest)) {
            Ok(m) if m.is_file() => m.len(),
            Ok(_) => {
                self.fail(
                    op,
                    OpError::with(
                        FailCode::ChangedSincePreview,
                        "destination is no longer a regular file",
                    ),
                );
                return Ok(());
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.fail(
                    op,
                    OpError::with(
                        FailCode::SourceMissing,
                        "file is no longer at the destination",
                    ),
                );
                return Ok(());
            }
            Err(e) => {
                self.fail(op, OpError::io(&e));
                return Ok(());
            }
        };
        if self.platform.is_locked(&dest) {
            self.fail(op, OpError::new(FailCode::Locked));
            return Ok(());
        }
        let current = match hash_file(self.platform, &dest) {
            Ok(h) => h,
            Err(e) => {
                self.fail(op, OpError::io(&e));
                return Ok(());
            }
        };
        let mut reasons = Vec::new();
        if op.hash_after.as_deref() != Some(current.as_str()) {
            reasons.push(ConfirmReason::Modified);
        }
        match op.op_type {
            OpType::Move => {
                let occupied = exists(self.platform, &op.source_path);
                if occupied {
                    reasons.push(ConfirmReason::SourceOccupied);
                }
                if !reasons.is_empty() && !self.force {
                    self.confirm(op, reasons);
                    return Ok(());
                }
                self.undo_move(op, &dest, &current, size, occupied).await
            }
            OpType::Copy => {
                if !exists(self.platform, &op.source_path) {
                    reasons.push(ConfirmReason::OriginalMissing);
                }
                if !reasons.is_empty() && !self.force {
                    self.confirm(op, reasons);
                    return Ok(());
                }
                self.undo_copy(op, &dest, &current, size).await
            }
            OpType::Trash => {
                self.fail(
                    op,
                    OpError::with(FailCode::Io, "trash operations are not undone by Valet"),
                );
                Ok(())
            }
        }
    }

    /// Moves the file at `dest` back to the original source path (or, when
    /// that is taken and `force` is set, to the next free name beside it).
    async fn undo_move(
        &mut self,
        op: &OpRecord,
        dest: &Path,
        current: &str,
        size: u64,
        occupied: bool,
    ) -> Result<()> {
        let target = if occupied {
            next_free_name(&op.source_path, |p| exists(self.platform, p))
        } else {
            op.source_path.clone()
        };
        let Some(dir) = target.parent().filter(|d| !d.as_os_str().is_empty()) else {
            self.fail(
                op,
                OpError::with(FailCode::InvalidPath, "original path has no folder"),
            );
            return Ok(());
        };
        if let Err(e) = fs::create_dir_all(self.platform.os_path(dir)) {
            self.fail(op, OpError::io(&e));
            return Ok(());
        }
        let same_volume = matches!(self.platform.same_volume(dest, dir), Ok(true));
        if !same_volume && !(self.platform.can_trash(dest) && self.platform.can_trash(dir)) {
            self.fail(op, OpError::new(FailCode::NoRecycleBin));
            return Ok(());
        }

        let batch = self.batch().await?;
        let id = self
            .steps
            .planned(
                &batch,
                self.seq,
                &NewOp {
                    op_type: OpType::Move,
                    source: dest.to_path_buf(),
                    dest: Some(target.clone()),
                    size: Some(size),
                    undo_of: Some(op.id),
                },
            )
            .await?;
        self.seq += 1;
        self.steps.start(id, Some(current)).await?;
        let t = Transfer {
            op_id: id,
            source: dest,
            dest: target,
            hash: current,
            size,
            on_collision: if self.force {
                OnCollision::NextFreeName
            } else {
                OnCollision::Fail
            },
        };
        let outcome = if same_volume {
            rename_move(&t, self.steps.journal, self.platform).await?
        } else {
            copy_transfer(&t, true, self.steps.journal, self.platform).await?
        };
        match outcome {
            Ok(final_path) => {
                self.steps.undo_done(id, op.id, Some(current)).await?;
                self.report.undone.push(op.id);
                if final_path != op.source_path {
                    self.report.restored_as.push(Restored {
                        op_id: op.id,
                        path: final_path,
                    });
                }
            }
            Err(e) => {
                self.steps.failed(id, &e).await?;
                if e.code == FailCode::Occupied {
                    // Taken between the check and the rename.
                    self.confirm(op, vec![ConfirmReason::SourceOccupied]);
                } else {
                    self.fail(op, e);
                }
            }
        }
        Ok(())
    }

    /// Sends the copy at `dest` to the Recycle Bin.
    async fn undo_copy(
        &mut self,
        op: &OpRecord,
        dest: &Path,
        current: &str,
        size: u64,
    ) -> Result<()> {
        if !self.platform.can_trash(dest) {
            self.fail(op, OpError::new(FailCode::NoRecycleBin));
            return Ok(());
        }
        let batch = self.batch().await?;
        let id = self
            .steps
            .planned(
                &batch,
                self.seq,
                &NewOp {
                    op_type: OpType::Trash,
                    source: dest.to_path_buf(),
                    dest: None,
                    size: Some(size),
                    undo_of: Some(op.id),
                },
            )
            .await?;
        self.seq += 1;
        self.steps.start(id, Some(current)).await?;
        match self.platform.trash(dest) {
            Ok(()) => {
                self.steps.undo_done(id, op.id, None).await?;
                self.report.undone.push(op.id);
            }
            Err(e) => {
                let err = OpError::with(FailCode::TrashFailed, e.to_string());
                self.steps.failed(id, &err).await?;
                self.fail(op, err);
            }
        }
        Ok(())
    }
}
