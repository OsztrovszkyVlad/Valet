//! Startup recovery: decides, from the disk, the outcome of every operation a
//! crash left `planned` or `in_progress`, without asking the user. Each file
//! ends up exactly once: at its source or at its destination. Files Valet must
//! remove on the way (a leftover temp copy, the source of a finished copy) go
//! to the Recycle Bin as journaled `trash` operations of the `recovery` batch.
//! Cloud placeholders are never hashed.

use anyhow::anyhow;

use super::*;
use crate::journal::{JournalError, OpRecord, OpState};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryDecision {
    pub op_id: i64,
    pub batch_id: String,
    pub op_type: OpType,
    pub previous_state: OpState,
    pub new_state: OpState,
    pub explanation: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryReport {
    /// The `recovery` batch; `None` when there was nothing to recover.
    pub batch_id: Option<String>,
    pub decisions: Vec<RecoveryDecision>,
    /// Batches a crash left open, now closed as interrupted.
    pub closed_batches: Vec<String>,
}

impl RecoveryReport {
    /// One line without paths, for INFO logs.
    pub fn log_line(&self) -> String {
        let n = |s: OpState| self.decisions.iter().filter(|d| d.new_state == s).count();
        format!(
            "recovery: {} operation(s) decided ({} done, {} failed), {} batch(es) closed",
            self.decisions.len(),
            n(OpState::Done),
            n(OpState::Failed),
            self.closed_batches.len()
        )
    }
}

/// What is at a destination path.
enum Seen {
    Missing,
    /// Content hash equals the expected one.
    Matches(String),
    Differs(String),
    Placeholder,
}

struct Recoverer<'a> {
    steps: Steps<'a>,
    platform: &'a dyn Platform,
    batch_id: String,
    seq: i64,
}

/// Recovers every unfinished operation and closes batches a crash left open.
/// Writes one `recovery` batch whose summary lists each decision.
pub async fn recover(journal: &Journal, platform: &dyn Platform) -> Result<RecoveryReport> {
    let ops = journal.unfinished_ops().await?;
    let open_batches = journal.unfinished_batches().await?;
    if ops.is_empty() && open_batches.is_empty() {
        return Ok(RecoveryReport::default());
    }
    let batch_id = journal.create_batch(BatchKind::Recovery).await?;
    let mut r = Recoverer {
        steps: Steps { journal },
        platform,
        batch_id: batch_id.clone(),
        seq: 0,
    };
    let mut decisions = Vec::with_capacity(ops.len());
    for op in &ops {
        let (new_state, explanation) = r.decide(op).await?;
        decisions.push(RecoveryDecision {
            op_id: op.id,
            batch_id: op.batch_id.clone(),
            op_type: op.op_type,
            previous_state: op.state,
            new_state,
            explanation,
        });
    }
    for b in &open_batches {
        journal
            .finish_batch(b, &format!("interrupted; recovered by batch {batch_id}"))
            .await?;
    }

    let mut summary: Vec<String> = decisions
        .iter()
        .map(|d| {
            format!(
                "operation {} ({}, batch {}): {} -> {}: {}",
                d.op_id, d.op_type, d.batch_id, d.previous_state, d.new_state, d.explanation
            )
        })
        .collect();
    if !open_batches.is_empty() {
        summary.push(format!("closed batches: {}", open_batches.join(", ")));
    }
    journal.finish_batch(&batch_id, &summary.join("\n")).await?;
    Ok(RecoveryReport {
        batch_id: Some(batch_id),
        decisions,
        closed_batches: open_batches,
    })
}

impl Recoverer<'_> {
    async fn decide(&mut self, op: &OpRecord) -> Result<(OpState, String)> {
        match (op.state, op.op_type) {
            (OpState::Planned, _) => self.fail(op, "never started; nothing was touched").await,
            (OpState::InProgress, OpType::Trash) => {
                if exists(self.platform, &op.source_path) {
                    self.fail(
                        op,
                        &format!(
                            "{} is still there; not sent to the Recycle Bin",
                            op.source_path.display()
                        ),
                    )
                    .await
                } else {
                    self.done(
                        op,
                        None,
                        "file no longer at its path; counted as sent to the Recycle Bin",
                    )
                    .await
                }
            }
            (OpState::InProgress, OpType::Move | OpType::Copy) => self.transfer(op).await,
            (state, _) => Err(anyhow!("operation {} is {state}, not unfinished", op.id)),
        }
    }

    async fn transfer(&mut self, op: &OpRecord) -> Result<(OpState, String)> {
        let Some(dest) = op.dest_path.clone() else {
            return self
                .fail(op, "no destination recorded; nothing was touched")
                .await;
        };
        let src = &op.source_path;
        let src_present = exists(self.platform, src);
        let seen = self.observe(&dest, op.hash_before.as_deref());
        let is_move = op.op_type == OpType::Move;

        match (seen, src_present) {
            (Seen::Matches(h), false) => {
                self.done(op, Some(&h), "verified at destination; source gone").await
            }
            (Seen::Matches(h), true) if !is_move => {
                self.done(op, Some(&h), "copy verified at destination").await
            }
            (Seen::Matches(h), true) => {
                // Crash after the copy was placed, before the source was trashed.
                let src_same = !self.platform.is_cloud_placeholder(src)
                    && hash_file(self.platform, src).ok().as_deref() == Some(h.as_str());
                if !src_same {
                    let note = self.trash_note(&dest).await?;
                    return self
                        .fail(
                            op,
                            &format!("source changed or unreadable; kept at source, copy at destination {note}"),
                        )
                        .await;
                }
                match self.trash(src).await? {
                    Ok(()) => {
                        self.done(op, Some(&h), "copy verified at destination; source sent to the Recycle Bin")
                            .await
                    }
                    Err(e) => {
                        let note = self.trash_note(&dest).await?;
                        self.fail_code(
                            op,
                            FailCode::TrashFailed,
                            &format!("source could not be sent to the Recycle Bin ({e}); kept at source, copy at destination {note}"),
                        )
                        .await
                    }
                }
            }
            (Seen::Placeholder, false) => {
                self.done(op, None, "at destination (cloud placeholder, not hashed); source gone")
                    .await
            }
            (Seen::Placeholder, true) if !is_move => {
                self.done(op, None, "copy at destination (cloud placeholder, not hashed)")
                    .await
            }
            (Seen::Placeholder, true) => {
                self.fail(
                    op,
                    "destination is a cloud placeholder and cannot be verified; left as is; file kept at source",
                )
                .await
            }
            (Seen::Missing, true) => {
                let note = self.trash_temp(op, &dest).await?;
                self.fail(op, &format!("not at destination; file kept at source{note}"))
                    .await
            }
            (Seen::Missing, false) => {
                let temp = dest.parent().map(|d| temp_path(d, op.id));
                let note = match temp {
                    Some(t) if exists(self.platform, &t) => {
                        format!("; copy kept at {}", t.display())
                    }
                    _ => String::new(),
                };
                self.fail(op, &format!("missing at source and destination{note}"))
                    .await
            }
            (Seen::Differs(why), true) => {
                let note = self.trash_temp(op, &dest).await?;
                self.fail(
                    op,
                    &format!("destination holds other content ({why}); left as is; file kept at source{note}"),
                )
                .await
            }
            (Seen::Differs(why), false) => {
                self.fail(
                    op,
                    &format!("source gone; destination differs from the journal ({why}); left as is"),
                )
                .await
            }
        }
    }

    fn observe(&self, path: &Path, expected: Option<&str>) -> Seen {
        if !exists(self.platform, path) {
            return Seen::Missing;
        }
        if self.platform.is_cloud_placeholder(path) {
            return Seen::Placeholder;
        }
        let Some(expected) = expected else {
            return Seen::Differs("no source hash recorded".into());
        };
        match hash_file(self.platform, path) {
            Ok(h) if h == expected => Seen::Matches(h),
            Ok(_) => Seen::Differs("content hash differs".into()),
            Err(e) => Seen::Differs(format!("cannot read: {e}")),
        }
    }

    /// Sends `path` to the Recycle Bin as a journaled `trash` operation.
    async fn trash(&mut self, path: &Path) -> Result<Result<(), String>> {
        let size = fs::symlink_metadata(self.platform.os_path(path))
            .ok()
            .map(|m| m.len());
        let id = self
            .steps
            .planned(
                &self.batch_id,
                self.seq,
                &NewOp {
                    op_type: OpType::Trash,
                    source: path.to_path_buf(),
                    dest: None,
                    size,
                    undo_of: None,
                },
            )
            .await?;
        self.seq += 1;
        self.steps.start(id, None).await?;
        match self.platform.trash(path) {
            Ok(()) => {
                self.steps.done(id, None).await?;
                Ok(Ok(()))
            }
            Err(e) => {
                self.steps.failed(id, &OpError::io(&e)).await?;
                Ok(Err(e.to_string()))
            }
        }
    }

    /// Trashes the copy at `path` and describes the result for an explanation.
    async fn trash_note(&mut self, path: &Path) -> Result<String> {
        Ok(match self.trash(path).await? {
            Ok(()) => "sent to the Recycle Bin".into(),
            Err(e) => format!(
                "could not be sent to the Recycle Bin ({e}) and is still at {}",
                path.display()
            ),
        })
    }

    /// Trashes a leftover `.valet-tmp-<id>` next to `dest` (only called while
    /// the source is present, so the temp is never the only copy).
    async fn trash_temp(&mut self, op: &OpRecord, dest: &Path) -> Result<String> {
        let Some(temp) = dest.parent().map(|d| temp_path(d, op.id)) else {
            return Ok(String::new());
        };
        if !exists(self.platform, &temp) {
            return Ok(String::new());
        }
        Ok(match self.trash(&temp).await? {
            Ok(()) => "; partial copy sent to the Recycle Bin".into(),
            Err(e) => format!(
                "; partial copy at {} could not be sent to the Recycle Bin ({e})",
                temp.display()
            ),
        })
    }

    async fn done(
        &mut self,
        op: &OpRecord,
        hash_after: Option<&str>,
        why: &str,
    ) -> Result<(OpState, String)> {
        match op.undo_of {
            Some(original) => match self.steps.undo_done(op.id, original, hash_after).await {
                Ok(()) => {}
                // The original is not `done` any more; record the undo step alone.
                Err(e) if e.downcast_ref::<JournalError>().is_some() => {
                    self.steps.done(op.id, hash_after).await?
                }
                Err(e) => return Err(e),
            },
            None => self.steps.done(op.id, hash_after).await?,
        }
        Ok((OpState::Done, why.to_owned()))
    }

    async fn fail(&mut self, op: &OpRecord, why: &str) -> Result<(OpState, String)> {
        self.fail_code(op, FailCode::Interrupted, why).await
    }

    async fn fail_code(
        &mut self,
        op: &OpRecord,
        code: FailCode,
        why: &str,
    ) -> Result<(OpState, String)> {
        self.steps.failed(op.id, &OpError::with(code, why)).await?;
        Ok((OpState::Failed, why.to_owned()))
    }
}
