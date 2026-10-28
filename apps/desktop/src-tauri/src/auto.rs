//! Automatic organizing (PLAN 2.3, 2.5, 4.3, 4.4).
//!
//! Inputs: batches of changed paths from the file watcher, and a full rescan
//! of every watched folder every [`RESCAN_INTERVAL`]. A pass plans the
//! watched folders that changed and executes only the operations of rules
//! approved at their current version under the watched folder of the file
//! (see `settings::Approvals`). Other operations (rule edited or never
//! previewed, or a folder added since the last preview) are left for the
//! next manual preview.
//!
//! A pass runs only when [`pass_allowed`]: `auto_apply_enabled` and
//! `first_preview_approved` in `app_settings.json`, not paused from the tray,
//! and no preview open. Passes hold `AppState.file_ops`, so they never overlap
//! each other, a manual batch or an undo.
//!
//! The scheduling and retry decisions are plain types ([`Scheduler`],
//! [`LockedRetries`]) with an injected clock, so they are tested without
//! Tauri or timers.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use valet_core::{
    executor::{BatchReport, FailCode, OpResult},
    planner::{PlannedOp, SkipReason, RECENT_WRITE_WINDOW},
    rules::Rule,
    storage::Db,
};

use crate::commands::operations::{execute_blocking, plan_blocking, record_stats};
use crate::commands::platform::notify_batch_done;
use crate::commands::settings::{
    load_app_settings, load_approvals, root_key, watched_folders, AppSettings, Approvals,
};
use crate::state::AppState;

/// Watcher batches arriving within this time after the first one go into
/// the same pass (the watcher itself already debounces 2 s).
pub(crate) const COALESCE: Duration = Duration::from_secs(2);

/// Full rescan of every watched folder this long after the last full pass.
pub(crate) const RESCAN_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// A pass triggered by watcher events that skipped a file as being written
/// (modified less than `RECENT_WRITE_WINDOW` ago, typically a download that
/// just finished) plans the same folders once more after this delay.
pub(crate) const RECHECK_DELAY: Duration = Duration::from_secs(RECENT_WRITE_WINDOW.as_secs() + 2);

/// A locked file is retried on this many passes after the pass where it was
/// first found locked; then it waits for its next change event.
pub(crate) const LOCKED_RETRY_PASSES: u32 = 3;

/// Tauri event emitted after an automatic batch that ran at least one
/// operation. Payload: [`BatchFinished`].
pub(crate) const BATCH_FINISHED_EVENT: &str = "batch-finished";

#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct BatchFinished {
    pub(crate) batch_id: String,
    pub(crate) done: usize,
    pub(crate) failed: usize,
    pub(crate) auto: bool,
}

/// Messages to the auto-apply loop.
#[derive(Debug)]
pub(crate) enum AutoMsg {
    /// A debounced watcher batch (a watched root itself means "rescan it").
    Changed(Vec<PathBuf>),
    /// The watched folders changed (`save_config`).
    Roots(Vec<PathBuf>),
}

/// The gating rule for every automatic pass.
pub(crate) fn pass_allowed(settings: &AppSettings, paused: bool, preview_active: bool) -> bool {
    settings.auto_apply_enabled && settings.first_preview_approved && !paused && !preview_active
}

fn gate_open(state: &AppState, cfg_dir: &Path, now: Instant) -> bool {
    pass_allowed(
        &load_app_settings(cfg_dir),
        state.paused.load(Ordering::SeqCst),
        state.preview_active(now),
    )
}

// ---------------------------------------------------------------- scheduling

/// What one pass should plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Work {
    /// Watched folders to plan, in configuration order.
    pub(crate) roots: Vec<PathBuf>,
    /// Paths the watcher reported since the previous pass.
    pub(crate) changed: Vec<PathBuf>,
    /// Watcher events contributed to this pass (not only the timer or a
    /// recheck). Only such a pass may schedule a recheck.
    pub(crate) from_events: bool,
}

/// Decides when passes run and which folders they plan. Time is passed in.
#[derive(Debug)]
pub(crate) struct Scheduler {
    roots: Vec<PathBuf>,
    pending: BTreeSet<PathBuf>,
    changed: Vec<PathBuf>,
    from_events: bool,
    /// When the pending folders are planned.
    ready_at: Option<Instant>,
    /// When the next full rescan is due.
    next_full: Instant,
}

impl Scheduler {
    pub(crate) fn new(roots: Vec<PathBuf>, now: Instant) -> Self {
        Self {
            roots,
            pending: BTreeSet::new(),
            changed: Vec::new(),
            from_events: false,
            ready_at: None,
            next_full: now + RESCAN_INTERVAL,
        }
    }

    fn ready_by(&mut self, at: Instant) {
        self.ready_at = Some(self.ready_at.map_or(at, |r| r.min(at)));
    }

    /// New watched folders: plan all of them soon.
    pub(crate) fn set_roots(&mut self, roots: Vec<PathBuf>, now: Instant) {
        self.roots = roots;
        self.pending = self.roots.iter().cloned().collect();
        self.ready_by(now + COALESCE);
    }

    /// A watcher batch. Each watched folder containing a path is planned at
    /// most [`COALESCE`] after the first batch that named it.
    pub(crate) fn on_changed(&mut self, paths: &[PathBuf], now: Instant) {
        let mut hit = false;
        for root in &self.roots {
            if paths.iter().any(|p| p.starts_with(root)) {
                self.pending.insert(root.clone());
                hit = true;
            }
        }
        if hit {
            self.changed.extend(paths.iter().cloned());
            self.from_events = true;
            self.ready_by(now + COALESCE);
        }
    }

    /// Plans `roots` again at `at` (recheck after a being-written skip).
    pub(crate) fn recheck(&mut self, roots: &[PathBuf], at: Instant) {
        let known: Vec<PathBuf> = roots
            .iter()
            .filter(|r| self.roots.contains(r))
            .cloned()
            .collect();
        if !known.is_empty() {
            self.pending.extend(known);
            self.ready_by(at);
        }
    }

    /// When [`Scheduler::take`] next has something.
    pub(crate) fn next_wake(&self) -> Instant {
        self.ready_at
            .map_or(self.next_full, |r| r.min(self.next_full))
    }

    /// The pass due at `now`, if any. When the full rescan is due, pending
    /// watcher work joins it (one pass, not two). A pass that plans every
    /// watched folder restarts the rescan interval.
    pub(crate) fn take(&mut self, now: Instant) -> Option<Work> {
        let full_due = now >= self.next_full;
        let ready = self.ready_at.is_some_and(|r| now >= r);
        if !full_due && !ready {
            return None;
        }
        let roots: Vec<PathBuf> = if full_due {
            self.roots.clone()
        } else {
            self.roots
                .iter()
                .filter(|r| self.pending.contains(*r))
                .cloned()
                .collect()
        };
        if full_due || roots.len() == self.roots.len() {
            self.next_full = now + RESCAN_INTERVAL;
        }
        self.pending.clear();
        self.ready_at = None;
        let changed = std::mem::take(&mut self.changed);
        let from_events = std::mem::take(&mut self.from_events);
        if roots.is_empty() {
            return None;
        }
        Some(Work {
            roots,
            changed,
            from_events,
        })
    }
}

// ---------------------------------------------------------------- locked files

/// Locked-file retries (PLAN 4.3): a file found locked in an automatic pass
/// is tried again on the next [`LOCKED_RETRY_PASSES`] passes, then left
/// until the watcher reports a change at or above its path. In memory only.
#[derive(Debug, Default)]
pub(crate) struct LockedRetries {
    /// Source path -> passes in which it was found locked.
    failures: HashMap<PathBuf, u32>,
}

impl LockedRetries {
    /// Paths the watcher reported: their counts (and those of files below
    /// them) start over.
    pub(crate) fn on_changed(&mut self, paths: &[PathBuf]) {
        if !paths.is_empty() {
            self.failures
                .retain(|k, _| !paths.iter().any(|p| k.starts_with(p)));
        }
    }

    /// `path` may be tried in this pass.
    pub(crate) fn allows(&self, path: &Path) -> bool {
        self.failures
            .get(path)
            .map_or(true, |&n| n <= LOCKED_RETRY_PASSES)
    }

    /// Counts the `Locked` failures of a pass; any other outcome clears the
    /// count for that file.
    pub(crate) fn record(&mut self, results: &[OpResult]) {
        for r in results {
            match &r.error {
                Some(e) if e.code == FailCode::Locked => {
                    *self.failures.entry(r.source.clone()).or_insert(0) += 1;
                }
                _ => {
                    self.failures.remove(&r.source);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- one pass

/// Result of a pass that was allowed to run.
#[derive(Debug)]
pub(crate) struct PassOutcome {
    /// The batch, when at least one operation was executed.
    pub(crate) report: Option<BatchReport>,
    /// The plan skipped a file as being written.
    pub(crate) being_written: bool,
    /// Operations left for the next manual preview (rule not approved at
    /// its current version under the file's watched folder).
    pub(crate) unapproved: usize,
}

/// Splits planned operations of a pass over `roots` into those it may
/// execute (rule approved at its current version under the watched folder of
/// the source, see [`Approvals::allows_path`]; file not out of locked
/// retries) and the number left because their rule is not approved there.
pub(crate) fn approved_ops(
    ops: Vec<PlannedOp>,
    rules: &[Rule],
    approvals: &Approvals,
    roots: &[PathBuf],
    retries: &LockedRetries,
) -> (Vec<PlannedOp>, usize) {
    let root_keys: Vec<String> = roots.iter().map(|r| root_key(r)).collect();
    let mut unapproved = 0;
    let run = ops
        .into_iter()
        .filter(|op| {
            let approved = rules
                .iter()
                .find(|r| r.id == op.rule_id)
                .is_some_and(|r| approvals.allows_path(r, &op.source, &root_keys));
            if !approved {
                unapproved += 1;
            }
            approved && retries.allows(&op.source)
        })
        .collect();
    (run, unapproved)
}

/// One automatic pass over `roots`. `Ok(None)` when the gate is closed
/// (checked before and again after taking `file_ops`). Planning and
/// execution run on blocking threads.
pub(crate) async fn run_pass(
    state: &AppState,
    db: &Db,
    cfg_dir: &Path,
    roots: &[PathBuf],
    retries: &mut LockedRetries,
    now: Instant,
) -> Result<Option<PassOutcome>, String> {
    if !gate_open(state, cfg_dir, now) {
        return Ok(None);
    }
    let _file_ops = state.file_ops.lock().await;
    // Settings, pause or preview may have changed while waiting.
    if !gate_open(state, cfg_dir, now) {
        return Ok(None);
    }

    let rules = db
        .list_rules()
        .await
        .map_err(|e| format!("Failed to load rules: {e}"))?;
    let approvals = load_approvals(cfg_dir);
    let plan = plan_blocking(roots.to_vec(), rules.clone(), state.platform.clone()).await?;
    let being_written = plan
        .skipped
        .iter()
        .any(|s| s.reason == SkipReason::BeingWritten);
    let (ops, unapproved) = approved_ops(plan.ops, &rules, &approvals, roots, retries);
    if ops.is_empty() {
        return Ok(Some(PassOutcome {
            report: None,
            being_written,
            unapproved,
        }));
    }

    let report = execute_blocking(ops, db.journal(), state.platform.clone()).await?;
    record_stats(db, &report).await;
    retries.record(&report.results);
    Ok(Some(PassOutcome {
        report: Some(report),
        being_written,
        unapproved,
    }))
}

// ---------------------------------------------------------------- wiring

/// Starts the watcher on the watched folders and the auto-apply loop. Call
/// once, after journal recovery.
pub(crate) fn start<R: Runtime>(app: &AppHandle<R>, cfg_dir: PathBuf) {
    let state = app.state::<AppState>().inner().clone();
    let (tx, rx) = unbounded_channel();
    *state
        .auto_tx
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(tx);
    let roots = watched_folders(&cfg_dir).unwrap_or_default();
    restart_watcher(&state, roots.clone());
    let app = app.clone();
    tauri::async_runtime::spawn(run_loop(app, state, cfg_dir, roots, rx));
}

/// Replaces the running watcher with one on `roots` (none when empty) and
/// tells the auto-apply loop about the new folders. Does nothing before
/// [`start`].
pub(crate) fn restart_watcher(state: &AppState, roots: Vec<PathBuf>) {
    let Some(tx) = state
        .auto_tx
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
    else {
        return;
    };
    state.stop_watcher();
    let _ = tx.send(AutoMsg::Roots(roots.clone()));
    if roots.is_empty() {
        log::info!("No watched folders, watcher not started");
        return;
    }
    log::info!("Starting watcher on {} folder(s)", roots.len());
    let slot = state.watcher.clone();
    tauri::async_runtime::spawn(async move {
        let result = valet_platform::watch_paths(roots, move |paths| {
            log::debug!("watcher event: {} path(s)", paths.len());
            let _ = tx.send(AutoMsg::Changed(paths));
        })
        .await;
        match result {
            Ok(handle) => {
                let previous = slot
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .replace(handle);
                // Two quick restarts: keep only the newest watcher.
                if let Some(previous) = previous {
                    previous.cancel();
                }
                log::info!("Watcher started");
            }
            Err(e) => {
                // The error chain names the folder that failed.
                log::error!("Failed to start watcher");
                log::debug!("watcher error: {:#}", e);
            }
        }
    });
}

async fn run_loop<R: Runtime>(
    app: AppHandle<R>,
    state: AppState,
    cfg_dir: PathBuf,
    roots: Vec<PathBuf>,
    mut rx: UnboundedReceiver<AutoMsg>,
) {
    let db = match Db::connect(&state._db_path).await {
        Ok(db) => db,
        Err(e) => {
            log::error!("Auto-apply stopped: database connection failed: {e}");
            return;
        }
    };
    let mut sched = Scheduler::new(roots, Instant::now());
    let mut retries = LockedRetries::default();
    loop {
        let wake = tokio::time::Instant::from_std(sched.next_wake());
        tokio::select! {
            msg = rx.recv() => match msg {
                Some(AutoMsg::Changed(paths)) => sched.on_changed(&paths, Instant::now()),
                Some(AutoMsg::Roots(roots)) => sched.set_roots(roots, Instant::now()),
                None => break,
            },
            _ = tokio::time::sleep_until(wake) => {}
        }
        while let Some(work) = sched.take(Instant::now()) {
            retries.on_changed(&work.changed);
            let outcome = match run_pass(
                &state,
                &db,
                &cfg_dir,
                &work.roots,
                &mut retries,
                Instant::now(),
            )
            .await
            {
                Ok(Some(o)) => o,
                Ok(None) => continue,
                Err(e) => {
                    log::warn!("Automatic pass failed: {e}");
                    continue;
                }
            };
            if outcome.being_written && work.from_events {
                sched.recheck(&work.roots, Instant::now() + RECHECK_DELAY);
            }
            if outcome.unapproved > 0 {
                log::info!(
                    "Automatic pass: {} operation(s) wait for a preview",
                    outcome.unapproved
                );
            }
            if let Some(report) = outcome.report {
                finished(&app, &report);
            }
        }
    }
}

fn finished<R: Runtime>(app: &AppHandle<R>, report: &BatchReport) {
    let done = report.done_count();
    let failed = report.failed_count();
    log::info!(
        "Automatic batch {}: {} done, {} failed",
        report.batch_id,
        done,
        failed
    );
    notify_batch_done(app, done, failed);
    let payload = BatchFinished {
        batch_id: report.batch_id.clone(),
        done,
        failed,
        auto: true,
    };
    if let Err(e) = app.emit(BATCH_FINISHED_EVENT, payload) {
        log::warn!("Failed to emit {BATCH_FINISHED_EVENT}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::SystemTime;

    use serde_json::json;
    use uuid::Uuid;
    use valet_core::executor::OpError;
    use valet_core::planner::OpKind;

    use crate::commands::settings::{record_preview_approval, PreviewApproval};

    /// Executing a preview of `roots` that contained `rules`.
    fn approve(cfg: &Path, rules: &[(Uuid, i32)], roots: &[&Path]) {
        let approval = PreviewApproval {
            rules: rules.to_vec(),
            roots: roots.iter().map(|r| r.to_path_buf()).collect(),
        };
        record_preview_approval(cfg, &approval).unwrap();
    }

    fn settings(auto: bool, approved: bool) -> AppSettings {
        AppSettings {
            auto_apply_enabled: auto,
            first_preview_approved: approved,
            ..AppSettings::default()
        }
    }

    #[test]
    fn gate_truth_table() {
        for auto in [false, true] {
            for approved in [false, true] {
                for paused in [false, true] {
                    for preview in [false, true] {
                        assert_eq!(
                            pass_allowed(&settings(auto, approved), paused, preview),
                            auto && approved && !paused && !preview,
                            "auto={auto} approved={approved} paused={paused} preview={preview}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn gate_reopens_after_preview_timeout() {
        let state = AppState::new(PathBuf::from("valet.sqlite3"));
        let tmp = tempfile::tempdir().unwrap();
        write_settings(tmp.path(), true, true);
        let t0 = Instant::now();
        assert!(gate_open(&state, tmp.path(), t0));
        state.open_preview(t0);
        assert!(!gate_open(&state, tmp.path(), t0 + Duration::from_secs(60)));
        assert!(gate_open(
            &state,
            tmp.path(),
            t0 + crate::state::PREVIEW_TIMEOUT
        ));
        state.close_preview();
        state.paused.store(true, Ordering::SeqCst);
        assert!(!gate_open(&state, tmp.path(), t0));
    }

    // ------------------------------------------------------------ scheduler

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn watcher_batches_coalesce_and_plan_only_changed_roots() {
        let t0 = Instant::now();
        let mut s = Scheduler::new(vec![p("/a"), p("/b"), p("/c")], t0);
        assert_eq!(s.take(t0), None);
        assert_eq!(s.next_wake(), t0 + RESCAN_INTERVAL);

        s.on_changed(&[p("/b/x.pdf")], t0);
        s.on_changed(&[p("/a/y.txt")], t0 + secs(1));
        s.on_changed(&[p("/elsewhere/z")], t0 + secs(1));
        assert_eq!(s.next_wake(), t0 + COALESCE, "first batch sets the time");
        assert_eq!(s.take(t0 + secs(1)), None);
        let w = s.take(t0 + COALESCE).unwrap();
        assert_eq!(w.roots, vec![p("/a"), p("/b")], "configuration order");
        assert_eq!(w.changed, vec![p("/b/x.pdf"), p("/a/y.txt")]);
        assert!(w.from_events);
        assert_eq!(s.take(t0 + COALESCE), None, "taken once");
        // A partial pass does not move the rescan.
        assert_eq!(s.next_wake(), t0 + RESCAN_INTERVAL);

        // A root in a batch (overflow) rescans that root.
        s.on_changed(&[p("/c")], t0 + secs(10));
        assert_eq!(s.take(t0 + secs(12)).unwrap().roots, vec![p("/c")]);
    }

    #[test]
    fn timer_rescans_everything_and_absorbs_pending_events() {
        let t0 = Instant::now();
        let mut s = Scheduler::new(vec![p("/a"), p("/b")], t0);
        let t = t0 + RESCAN_INTERVAL - secs(1);
        s.on_changed(&[p("/a/f")], t);
        // The timer is due before the coalesce window ends: one full pass.
        let w = s.take(t0 + RESCAN_INTERVAL).unwrap();
        assert_eq!(w.roots, vec![p("/a"), p("/b")]);
        assert_eq!(w.changed, vec![p("/a/f")]);
        assert!(w.from_events);
        assert_eq!(s.take(t + COALESCE), None, "no second pass for the event");
        assert_eq!(s.next_wake(), t0 + RESCAN_INTERVAL * 2);

        // Timer only: no events.
        let w = s.take(t0 + RESCAN_INTERVAL * 2).unwrap();
        assert!(!w.from_events && w.changed.is_empty());
    }

    #[test]
    fn full_pass_from_events_restarts_the_rescan_interval() {
        let t0 = Instant::now();
        let mut s = Scheduler::new(vec![p("/a"), p("/b")], t0);
        let t = t0 + secs(300);
        s.on_changed(&[p("/a"), p("/b/f")], t);
        assert_eq!(s.take(t + COALESCE).unwrap().roots.len(), 2);
        assert_eq!(s.next_wake(), t + COALESCE + RESCAN_INTERVAL);
        assert_eq!(s.take(t0 + RESCAN_INTERVAL), None, "not due any more");
    }

    #[test]
    fn recheck_and_new_roots() {
        let t0 = Instant::now();
        let mut s = Scheduler::new(vec![p("/a"), p("/b")], t0);
        s.recheck(&[p("/a"), p("/gone")], t0 + RECHECK_DELAY);
        assert_eq!(s.take(t0 + secs(5)), None);
        let w = s.take(t0 + RECHECK_DELAY).unwrap();
        assert_eq!(w.roots, vec![p("/a")]);
        assert!(!w.from_events, "a recheck does not schedule another");

        // An event before a pending recheck brings it forward.
        s.recheck(&[p("/a")], t0 + secs(100));
        s.on_changed(&[p("/b/x")], t0 + secs(90));
        let w = s.take(t0 + secs(92)).unwrap();
        assert_eq!(w.roots, vec![p("/a"), p("/b")]);

        // New folders are planned soon; folders no longer watched are not.
        s.on_changed(&[p("/a/x")], t0 + secs(200));
        s.set_roots(vec![p("/c")], t0 + secs(200));
        assert_eq!(s.take(t0 + secs(202)).unwrap().roots, vec![p("/c")]);
        s.set_roots(vec![], t0 + secs(300));
        assert_eq!(s.take(t0 + secs(302)), None);
        assert_eq!(s.take(t0 + secs(300) + RESCAN_INTERVAL * 2), None);
    }

    // ------------------------------------------------------------ locked retries

    fn locked(path: &Path) -> OpResult {
        outcome(path, Some(FailCode::Locked))
    }

    fn outcome(path: &Path, code: Option<FailCode>) -> OpResult {
        OpResult {
            op_id: 1,
            kind: OpKind::Move,
            source: path.to_path_buf(),
            dest: PathBuf::from("out"),
            rule_name: "r".to_string(),
            size: 1,
            error: code.map(|code| OpError { code, detail: None }),
        }
    }

    #[test]
    fn locked_file_is_retried_on_the_next_three_passes_then_left() {
        let f = p("/in/a.docx");
        let mut r = LockedRetries::default();
        let mut attempts = 0;
        for _pass in 0..10 {
            if r.allows(&f) {
                attempts += 1;
                r.record(&[locked(&f)]);
            }
        }
        assert_eq!(attempts, 1 + LOCKED_RETRY_PASSES);
        assert!(!r.allows(&f));

        // Its next change event starts over; so does a change of its folder.
        r.on_changed(&[p("/in/other.txt")]);
        assert!(!r.allows(&f));
        r.on_changed(std::slice::from_ref(&f));
        assert!(r.allows(&f));
        for _ in 0..4 {
            r.record(&[locked(&f)]);
        }
        assert!(!r.allows(&f));
        r.on_changed(&[p("/in")]);
        assert!(r.allows(&f));
    }

    #[test]
    fn locked_count_clears_when_the_file_moves_or_fails_otherwise() {
        let f = p("/in/a.docx");
        let mut r = LockedRetries::default();
        r.record(&[locked(&f), locked(&f), locked(&f)]);
        r.record(&[outcome(&f, None)]);
        for _ in 0..LOCKED_RETRY_PASSES {
            r.record(&[locked(&f)]);
        }
        assert!(r.allows(&f), "count restarted after the success");
        r.record(&[outcome(&f, Some(FailCode::SourceMissing))]);
        assert!(r.failures.is_empty());
    }

    // ------------------------------------------------------------ passes on disk

    fn write_settings(dir: &Path, auto: bool, approved: bool) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("app_settings.json"),
            serde_json::to_string(&settings(auto, approved)).unwrap(),
        )
        .unwrap();
    }

    fn move_rule(n: u128, ext: &str, dest: &Path, version: i32) -> Rule {
        serde_json::from_value(json!({
            "id": Uuid::from_u128(n),
            "name": format!("rule {n}"),
            "enabled": true,
            "conditions": [{ "type": "ext", "value": ext }],
            "actions": [{ "type": "moveTo", "params": { "path": dest } }],
            "version": version,
        }))
        .unwrap()
    }

    /// A file old enough not to count as being written.
    fn old_file(path: &Path, content: &[u8]) {
        std::fs::write(path, content).unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
    }

    struct Fixture {
        _tmp: tempfile::TempDir,
        inbox: PathBuf,
        out: PathBuf,
        cfg: PathBuf,
        state: AppState,
        db: Db,
    }

    async fn fixture() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let inbox = tmp.path().join("inbox");
        let out = tmp.path().join("out");
        let cfg = tmp.path().join("cfg");
        std::fs::create_dir_all(&inbox).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        let state = AppState::new(cfg.join("valet.sqlite3"));
        let db = Db::connect(&cfg.join("valet.sqlite3")).await.unwrap();
        write_settings(&cfg, true, true);
        Fixture {
            _tmp: tmp,
            inbox,
            out,
            cfg,
            state,
            db,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pass_executes_only_rules_approved_at_their_current_version() {
        let f = fixture().await;
        let approved = move_rule(1, "txt", &f.out.join("txt"), 1);
        let never = move_rule(2, "pdf", &f.out.join("pdf"), 1);
        let edited = move_rule(3, "jpg", &f.out.join("jpg"), 2);
        for r in [&approved, &never, &edited] {
            f.db.upsert_rule(r).await.unwrap();
        }
        // Rule 3 was approved at version 1, then edited.
        approve(&f.cfg, &[(approved.id, 1), (edited.id, 1)], &[&f.inbox]);
        old_file(&f.inbox.join("a.txt"), b"a");
        old_file(&f.inbox.join("b.pdf"), b"b");
        old_file(&f.inbox.join("c.jpg"), b"c");

        let mut retries = LockedRetries::default();
        let outcome = run_pass(
            &f.state,
            &f.db,
            &f.cfg,
            std::slice::from_ref(&f.inbox),
            &mut retries,
            Instant::now(),
        )
        .await
        .unwrap()
        .unwrap();
        let report = outcome.report.unwrap();
        assert_eq!(report.done_count(), 1);
        assert_eq!(report.results.len(), 1);
        assert_eq!(outcome.unapproved, 2);
        assert_eq!(
            std::fs::read(f.out.join("txt").join("a.txt")).unwrap(),
            b"a"
        );
        assert!(f.inbox.join("b.pdf").exists() && f.inbox.join("c.jpg").exists());
        assert_eq!(f.db.get_recent_operations(10).await.unwrap().len(), 1);
        assert_eq!(f.db.journal().list_batches(10).await.unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn closed_gate_plans_and_moves_nothing() {
        let f = fixture().await;
        let rule = move_rule(1, "txt", &f.out, 1);
        f.db.upsert_rule(&rule).await.unwrap();
        approve(&f.cfg, &[(rule.id, 1)], &[&f.inbox]);
        old_file(&f.inbox.join("a.txt"), b"a");
        let roots = vec![f.inbox.clone()];
        let mut retries = LockedRetries::default();
        let now = Instant::now();

        for (auto, approved) in [(false, true), (true, false)] {
            write_settings(&f.cfg, auto, approved);
            let r = run_pass(&f.state, &f.db, &f.cfg, &roots, &mut retries, now);
            assert!(r.await.unwrap().is_none());
        }
        write_settings(&f.cfg, true, true);
        f.state.paused.store(true, Ordering::SeqCst);
        let r = run_pass(&f.state, &f.db, &f.cfg, &roots, &mut retries, now);
        assert!(r.await.unwrap().is_none());
        f.state.paused.store(false, Ordering::SeqCst);
        f.state.open_preview(now);
        let r = run_pass(&f.state, &f.db, &f.cfg, &roots, &mut retries, now);
        assert!(r.await.unwrap().is_none());
        assert!(f.inbox.join("a.txt").exists());
        assert!(f.db.journal().list_batches(10).await.unwrap().is_empty());

        // After the preview times out the pass runs.
        let later = now + crate::state::PREVIEW_TIMEOUT;
        let r = run_pass(&f.state, &f.db, &f.cfg, &roots, &mut retries, later);
        assert_eq!(r.await.unwrap().unwrap().report.unwrap().done_count(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn passes_never_overlap() {
        let f = fixture().await;
        let rule = move_rule(1, "txt", &f.out, 1);
        f.db.upsert_rule(&rule).await.unwrap();
        approve(&f.cfg, &[(rule.id, 1)], &[&f.inbox]);
        for i in 0..20 {
            old_file(&f.inbox.join(format!("{i}.txt")), b"x");
        }
        let roots = vec![f.inbox.clone()];

        // A batch in progress (manual execute, undo or another pass) holds
        // `file_ops`: the pass waits for it.
        let guard = f.state.file_ops.lock().await;
        let task = {
            let (state, cfg, roots) = (f.state.clone(), f.cfg.clone(), roots.clone());
            let db = Db::connect(&cfg.join("valet.sqlite3")).await.unwrap();
            tokio::spawn(async move {
                let mut retries = LockedRetries::default();
                run_pass(&state, &db, &cfg, &roots, &mut retries, Instant::now()).await
            })
        };
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!task.is_finished());
        assert!(f.inbox.join("0.txt").exists());
        drop(guard);
        let first = task.await.unwrap().unwrap().unwrap();
        assert_eq!(first.report.unwrap().done_count(), 20);

        // Two passes started together: one moves every file, the other
        // finds nothing left; every file exists once.
        for i in 0..20 {
            old_file(&f.inbox.join(format!("n{i}.txt")), b"y");
        }
        let (mut r1, mut r2) = (LockedRetries::default(), LockedRetries::default());
        let now = Instant::now();
        let (a, b) = tokio::join!(
            run_pass(&f.state, &f.db, &f.cfg, &roots, &mut r1, now),
            run_pass(&f.state, &f.db, &f.cfg, &roots, &mut r2, now),
        );
        let reports: Vec<BatchReport> = [a, b]
            .into_iter()
            .filter_map(|r| r.unwrap().unwrap().report)
            .collect();
        assert_eq!(reports.len(), 1, "the second pass had nothing to do");
        assert_eq!(reports[0].done_count(), 20);
        assert_eq!(std::fs::read_dir(&f.inbox).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&f.out).unwrap().count(), 40);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn folder_added_after_the_preview_waits_for_a_new_one() {
        let f = fixture().await;
        let rule = move_rule(1, "txt", &f.out, 1);
        f.db.upsert_rule(&rule).await.unwrap();
        // The preview covered the inbox only.
        approve(&f.cfg, &[(rule.id, 1)], &[&f.inbox]);
        let added = f._tmp.path().join("added");
        std::fs::create_dir_all(&added).unwrap();
        old_file(&f.inbox.join("a.txt"), b"a");
        old_file(&added.join("b.txt"), b"b");
        let roots = vec![f.inbox.clone(), added.clone()];

        let mut retries = LockedRetries::default();
        let outcome = run_pass(
            &f.state,
            &f.db,
            &f.cfg,
            &roots,
            &mut retries,
            Instant::now(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(outcome.report.unwrap().done_count(), 1);
        assert_eq!(outcome.unapproved, 1);
        assert_eq!(std::fs::read(f.out.join("a.txt")).unwrap(), b"a");
        assert_eq!(
            std::fs::read(added.join("b.txt")).unwrap(),
            b"b",
            "not previewed"
        );

        // A preview of both folders approves the new one.
        approve(&f.cfg, &[(rule.id, 1)], &[&f.inbox, &added]);
        let outcome = run_pass(
            &f.state,
            &f.db,
            &f.cfg,
            &roots,
            &mut retries,
            Instant::now(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(outcome.report.unwrap().done_count(), 1);
        assert_eq!(outcome.unapproved, 0);
        assert!(!added.join("b.txt").exists());
        assert_eq!(std::fs::read(f.out.join("b.txt")).unwrap(), b"b");
    }

    #[test]
    fn nested_folders_need_approval_under_each_containing_folder() {
        let rule = move_rule(1, "txt", Path::new("/out"), 1);
        let op = |name: &str| PlannedOp {
            source: p(name),
            dest: p("/out/x"),
            kind: OpKind::Move,
            rule_id: rule.id,
            rule_name: rule.name.clone(),
            size: 1,
            mtime: SystemTime::UNIX_EPOCH,
        };
        let tmp = tempfile::tempdir().unwrap();
        // Only the outer folder was previewed.
        approve(tmp.path(), &[(rule.id, 1)], &[Path::new("/in")]);
        let approvals = load_approvals(tmp.path());
        let ops = || vec![op("/in/a.txt"), op("/in/sub/b.txt"), op("/in2/c.txt")];
        let run = |roots: &[PathBuf]| {
            let (run, unapproved) = approved_ops(
                ops(),
                std::slice::from_ref(&rule),
                &approvals,
                roots,
                &LockedRetries::default(),
            );
            let names: Vec<PathBuf> = run.into_iter().map(|o| o.source).collect();
            (names, unapproved)
        };
        assert_eq!(
            run(&[p("/in")]),
            (vec![p("/in/a.txt"), p("/in/sub/b.txt")], 1),
            "/in2 is not inside /in: a file outside every root of the pass is never approved"
        );
        // `/in/sub` added as its own watched folder: its files wait.
        assert_eq!(run(&[p("/in"), p("/in/sub")]), (vec![p("/in/a.txt")], 2));
    }

    #[test]
    fn approved_ops_drops_files_out_of_locked_retries() {
        let rule = move_rule(1, "txt", Path::new("/out"), 1);
        let mut approvals = Approvals::default();
        approvals.0.insert(
            rule.id,
            crate::commands::settings::Approval {
                version: 1,
                roots: [root_key(Path::new("/in"))].into(),
            },
        );
        let op = |name: &str| PlannedOp {
            source: p(name),
            dest: p("/out/x"),
            kind: OpKind::Move,
            rule_id: rule.id,
            rule_name: rule.name.clone(),
            size: 1,
            mtime: SystemTime::UNIX_EPOCH,
        };
        let mut retries = LockedRetries::default();
        for _ in 0..=LOCKED_RETRY_PASSES {
            retries.record(&[locked(Path::new("/in/locked.txt"))]);
        }
        let (run, unapproved) = approved_ops(
            vec![op("/in/locked.txt"), op("/in/free.txt")],
            std::slice::from_ref(&rule),
            &approvals,
            &[p("/in")],
            &retries,
        );
        assert_eq!(unapproved, 0);
        assert_eq!(
            run.iter().map(|o| o.source.clone()).collect::<Vec<_>>(),
            vec![p("/in/free.txt")]
        );
    }

    #[test]
    fn batch_finished_payload() {
        let v = serde_json::to_value(BatchFinished {
            batch_id: "b-1".to_string(),
            done: 2,
            failed: 1,
            auto: true,
        })
        .unwrap();
        assert_eq!(
            v,
            json!({ "batch_id": "b-1", "done": 2, "failed": 1, "auto": true })
        );
    }
}
