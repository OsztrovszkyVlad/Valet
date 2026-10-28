//! Operation commands: dry run, execute, statistics, history.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use tauri::{Emitter, Manager};
use valet_core::{
    executor::{self, BatchReport, UndoReport},
    journal::{BatchInfo, Journal, OpRecord},
    planner::{self, OpKind, Plan, PlanContext, PlannedOp},
    platform::Platform,
    rules::Rule,
    storage::Db,
};

use crate::commands::platform::notify_batch_done;
use crate::commands::settings::{record_preview_approval, watched_folders, RuleVersion};
use crate::state::{config_dir, AppState};

/// Plans `roots` with `rules` on a blocking thread: the planner walks
/// directories and may hash files, which must not stall the async runtime.
pub(crate) async fn plan_blocking(
    roots: Vec<PathBuf>,
    rules: Vec<Rule>,
    platform: Arc<dyn Platform>,
) -> Result<Plan, String> {
    tokio::task::spawn_blocking(move || {
        planner::plan(
            &roots,
            &rules,
            platform.as_ref(),
            &PlanContext::from_system(),
        )
    })
    .await
    .map_err(|e| format!("Planning stopped: {e}"))
}

/// Runs `executor::execute` on a blocking thread (file I/O and hashing run
/// inline in the executor; the journal calls are driven by the runtime
/// handle).
pub(crate) async fn execute_blocking(
    ops: Vec<PlannedOp>,
    journal: Journal,
    platform: Arc<dyn Platform>,
) -> Result<BatchReport, String> {
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        handle.block_on(executor::execute(&ops, &journal, platform.as_ref()))
    })
    .await
    .map_err(|e| format!("Execution stopped: {e}"))?
    .map_err(|e| format!("Operation journal could not be written: {:#}", e))
}

/// The enabled rules a plan contains, at their current version: rules with
/// an operation or a report entry (by id), and rules named by a skipped
/// entry (skips carry only the rule name). Executing the plan approves them.
pub(crate) fn rules_in_plan(plan: &Plan, rules: &[Rule]) -> Vec<RuleVersion> {
    let ids: HashSet<uuid::Uuid> = plan
        .ops
        .iter()
        .map(|o| o.rule_id)
        .chain(plan.reports.iter().map(|r| r.rule_id))
        .collect();
    let skip_names: HashSet<&str> = plan
        .skipped
        .iter()
        .filter_map(|s| s.rule_name.as_deref())
        .collect();
    rules
        .iter()
        .filter(|r| r.enabled && (ids.contains(&r.id) || skip_names.contains(r.name.as_str())))
        .map(|r| (r.id, r.version))
        .collect()
}

/// Writes one statistics row per result (as `execute_operations` always did).
pub(crate) async fn record_stats(db: &Db, report: &BatchReport) {
    for r in &report.results {
        let error = r.error.as_ref().map(|e| e.journal_text());
        if let Err(e) = db
            .record_operation(
                &r.source.to_string_lossy(),
                &r.dest.to_string_lossy(),
                op_type(r.kind),
                &r.rule_name,
                if error.is_none() { "success" } else { "failed" },
                error.as_deref(),
            )
            .await
        {
            log::warn!("Failed to record operation: {}", e);
        }
    }
}

fn op_type(kind: OpKind) -> &'static str {
    match kind {
        OpKind::Move => "move",
        OpKind::Copy => "copy",
        OpKind::Trash => "trash",
    }
}

#[derive(serde::Serialize)]
struct PerformanceMetrics {
    total_files_processed: usize,
    total_time_ms: u128,
    files_per_second: f32,
    bytes_processed: u64,
    errors_count: usize,
}

#[tauri::command]
pub(crate) async fn dry_run(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    log::info!("Running dry-run on configured paths");

    let state = app.state::<AppState>();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    // Watched folders that exist; `config.noFolders` when there are none.
    let paths = watched_folders(&config_dir(&app)?)?;

    let rules = db
        .list_rules()
        .await
        .map_err(|e| format!("Dry run failed: Failed to load rules: {}", e))?;
    // Auto-apply waits while the preview is open (from before planning, so
    // no pass moves files between the plan and the user's decision).
    state.open_preview(Instant::now());
    let plan = plan_blocking(paths.clone(), rules.clone(), state.platform.clone())
        .await
        .map_err(|e| {
            state.close_preview();
            format!("Dry run failed: {}", e)
        })?;
    let plan_rules = rules_in_plan(&plan, &rules);

    log::info!(
        "Dry-run completed: {} operation(s) proposed, {} skipped, across {} path(s)",
        plan.ops.len(),
        plan.skipped.len(),
        paths.len()
    );

    let plan_id = uuid::Uuid::new_v4().to_string();
    let json = dry_run_json(&plan, &plan_id);
    state.store_preview(plan_id, plan, plan_rules);
    Ok(json)
}

/// The shape `DryRunView.tsx` reads, plus `skipped` (path, reason, rule_name),
/// `reports` (path, rule_id, rule_name, size, group) and `plan_id`, which
/// `execute_operations` needs. A `trash` operation has an empty
/// `destination_path`.
fn dry_run_json(plan: &Plan, plan_id: &str) -> serde_json::Value {
    let results: Vec<serde_json::Value> = plan
        .ops
        .iter()
        .map(|op| {
            serde_json::json!({
              "source_path": op.source.to_string_lossy(),
              "destination_path": op.dest.to_string_lossy(),
              "rule_name": op.rule_name,
              "operation": op_type(op.kind),
            })
        })
        .collect();
    let skipped: Vec<serde_json::Value> = plan
        .skipped
        .iter()
        .map(|s| {
            serde_json::json!({
              "path": s.path.to_string_lossy(),
              "reason": s.reason,
              "rule_name": s.rule_name,
            })
        })
        .collect();

    let reports: Vec<serde_json::Value> = plan
        .reports
        .iter()
        .map(|r| {
            serde_json::json!({
              "path": r.path.to_string_lossy(),
              "rule_id": r.rule_id,
              "rule_name": r.rule_name,
              "size": r.size,
              "group": r.group,
            })
        })
        .collect();

    serde_json::json!({
      "total_files": results.len(),
      "total_rules_applied": results.len(),
      "results": results,
      "skipped": skipped,
      "reports": reports,
      "plan_id": plan_id,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub(crate) struct ExecuteResponse {
    pub(crate) executed_count: usize,
    failed_operations: Vec<ExecuteError>,
    success: bool,
    /// Journal batch of this run; pass it to `undo_batch`.
    batch_id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
struct ExecuteError {
    source_path: String,
    destination_path: String,
    rule_name: String,
    error_message: String,
}

/// The fields `DryRunView.tsx` reads, plus `batch_id`.
fn execute_response(report: &BatchReport) -> ExecuteResponse {
    let failed_operations: Vec<ExecuteError> = report
        .results
        .iter()
        .filter_map(|r| {
            r.error.as_ref().map(|e| ExecuteError {
                source_path: r.source.to_string_lossy().into_owned(),
                destination_path: r.dest.to_string_lossy().into_owned(),
                rule_name: r.rule_name.clone(),
                error_message: e.journal_text(),
            })
        })
        .collect();
    ExecuteResponse {
        executed_count: report.done_count(),
        success: failed_operations.is_empty(),
        failed_operations,
        batch_id: report.batch_id.clone(),
    }
}

/// Runs exactly the operations of the previewed plan `plan_id` and records
/// each result in the statistics table. Files that appeared after the preview
/// are not in the plan and are not touched; the executor re-checks every
/// planned file (missing, changed, placeholder, locked) before acting on it.
/// The stored plan is consumed: a second call with the same id, a wrong id or
/// no preview returns `PREVIEW_REQUIRED` and touches nothing.
/// `before_execute` runs once the plan is taken, before any file operation.
/// After the batch the rules the preview contained are approved at their
/// previewed version (`approvals.json` in `cfg_dir`) and
/// `first_preview_approved` is set; failing to write that is logged only.
pub(crate) async fn execute_previewed(
    state: &AppState,
    db: &Db,
    cfg_dir: &Path,
    plan_id: &str,
    before_execute: impl FnOnce(&Plan),
) -> Result<BatchReport, String> {
    // One batch at a time.
    let _file_ops = state.file_ops.lock().await;
    let (plan, plan_rules) = state.take_preview(plan_id)?;
    before_execute(&plan);

    let report = execute_blocking(plan.ops, db.journal(), state.platform.clone()).await?;
    record_stats(db, &report).await;
    // Still under `file_ops`, so an auto-apply pass sees all or none of it.
    if record_preview_approval(cfg_dir, &plan_rules).is_err() {
        log::warn!("Rule approvals could not be saved");
    }
    Ok(report)
}

/// Executes the plan the last `dry_run` returned. `plan_id` is that
/// response's `plan_id`; without a matching preview the error is
/// `"preview_required"`. Closes the preview (auto-apply may run again).
#[tauri::command]
pub(crate) async fn execute_operations(
    plan_id: String,
    app: tauri::AppHandle,
) -> Result<ExecuteResponse, String> {
    log::info!("Executing the previewed plan");

    let start_time = std::time::Instant::now();
    let state = app.state::<AppState>();
    state.close_preview();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    let report = execute_previewed(&state, &db, &config_dir(&app)?, &plan_id, |plan| {
        log::info!("Executing {} file operation(s)", plan.ops.len());
    })
    .await?;

    let response = execute_response(&report);
    let executed_count = response.executed_count;
    let failed_count = response.failed_operations.len();
    let total_time = start_time.elapsed();
    let total_bytes_processed: u64 = report
        .results
        .iter()
        .filter(|r| r.error.is_none())
        .map(|r| r.size)
        .sum();
    let files_per_second = if total_time.as_secs_f32() > 0.0 {
        executed_count as f32 / total_time.as_secs_f32()
    } else {
        0.0
    };
    let performance_metrics = PerformanceMetrics {
        total_files_processed: executed_count,
        total_time_ms: total_time.as_millis(),
        files_per_second,
        bytes_processed: total_bytes_processed,
        errors_count: failed_count,
    };

    log::info!(
        "Batch {}: {}/{} done, {} failed in {}ms",
        report.batch_id,
        executed_count,
        report.results.len(),
        failed_count,
        performance_metrics.total_time_ms
    );

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("operation-complete", &performance_metrics);
    }

    // Short notification in the UI language; nothing for an empty batch.
    notify_batch_done(&app, executed_count, failed_count);

    Ok(response)
}

/// The user left the preview without executing it: auto-apply may run
/// again. The stored plan stays executable.
#[tauri::command]
pub(crate) async fn close_preview(app: tauri::AppHandle) -> Result<(), String> {
    app.state::<AppState>().close_preview();
    Ok(())
}

/// Default number of batches returned by `list_batches`.
const DEFAULT_BATCH_LIMIT: u32 = 50;

/// Journal batches, newest first, with operation counts per state.
#[tauri::command]
pub(crate) async fn list_batches(
    limit: Option<u32>,
    app: tauri::AppHandle,
) -> Result<Vec<BatchInfo>, String> {
    let state = app.state::<AppState>();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;
    db.journal()
        .list_batches(limit.unwrap_or(DEFAULT_BATCH_LIMIT))
        .await
        .map_err(|e| format!("Failed to list batches: {}", e))
}

#[derive(serde::Serialize, Debug)]
pub(crate) struct BatchDetail {
    batch: BatchInfo,
    /// In `seq` order, with state and error.
    operations: Vec<OpRecord>,
}

/// One batch with its operations.
#[tauri::command]
pub(crate) async fn get_batch(
    batch_id: String,
    app: tauri::AppHandle,
) -> Result<BatchDetail, String> {
    let state = app.state::<AppState>();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;
    let journal = db.journal();
    let batch = journal
        .get_batch(&batch_id)
        .await
        .map_err(|e| format!("Failed to read batch: {}", e))?
        .ok_or_else(|| format!("Batch {} not found", batch_id))?;
    let operations = journal
        .list_ops(&batch_id)
        .await
        .map_err(|e| format!("Failed to read operations: {}", e))?;
    Ok(BatchDetail { batch, operations })
}

/// Undoes an apply batch. Operations whose files changed, or whose original
/// path is taken, come back in `needs_confirmation`; call again with
/// `force = true` after the user confirms.
#[tauri::command]
pub(crate) async fn undo_batch(
    batch_id: String,
    force: bool,
    app: tauri::AppHandle,
) -> Result<UndoReport, String> {
    let state = app.state::<AppState>();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;
    let _file_ops = state.file_ops.lock().await;
    let report = executor::undo_batch(&batch_id, &db.journal(), state.platform.as_ref(), force)
        .await
        .map_err(|e| format!("Undo failed: {:#}", e))?;
    log::info!(
        "Undo of batch {}: {} undone, {} need confirmation, {} failed, {} skipped",
        batch_id,
        report.undone.len(),
        report.needs_confirmation.len(),
        report.failed.len(),
        report.skipped.len()
    );
    Ok(report)
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct StatsResponse {
    stats: valet_core::model::OperationStats,
    recent_operations: Vec<valet_core::model::RecentOperation>,
}

#[tauri::command]
pub(crate) async fn get_statistics(
    time_range: String,
    app: tauri::AppHandle,
) -> Result<StatsResponse, String> {
    log::info!("Getting statistics for time range: {}", time_range);

    let state = app.state::<AppState>();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    let days_back = match time_range.as_str() {
        "1d" => Some(1),
        "7d" => Some(7),
        "30d" => Some(30),
        "90d" => Some(90),
        "all" => None,
        _ => Some(7), // default to 7 days
    };

    let stats = db
        .get_operation_statistics(days_back)
        .await
        .map_err(|e| format!("Failed to get statistics: {}", e))?;

    let recent_operations = db
        .get_recent_operations(50)
        .await
        .map_err(|e| format!("Failed to get recent operations: {}", e))?;

    Ok(StatsResponse {
        stats,
        recent_operations,
    })
}

#[tauri::command]
pub(crate) async fn clear_operation_history(app: tauri::AppHandle) -> Result<(), String> {
    log::info!("Clearing operation history");

    let state = app.state::<AppState>();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    db.clear_operation_history()
        .await
        .map_err(|e| format!("Failed to clear operation history: {}", e))?;

    log::info!("Operation history cleared successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use valet_core::executor::{FailCode, OpError, OpResult};

    use crate::state::PREVIEW_REQUIRED;

    /// A move of `source` to `dest` as the planner would record it now.
    fn planned_move(source: &Path, dest: &Path) -> PlannedOp {
        let meta = std::fs::metadata(source).unwrap();
        PlannedOp {
            source: source.to_path_buf(),
            dest: dest.to_path_buf(),
            kind: OpKind::Move,
            rule_id: uuid::Uuid::nil(),
            rule_name: "r".to_string(),
            size: meta.len(),
            mtime: meta.modified().unwrap(),
        }
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
        std::fs::create_dir_all(&inbox).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        let db_path = tmp.path().join("db").join("valet.sqlite3");
        let state = AppState::new(db_path.clone());
        let db = Db::connect(&db_path).await.unwrap();
        let cfg = tmp.path().join("cfg");
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
    async fn execute_without_matching_preview_touches_nothing() {
        let f = fixture().await;
        let a = f.inbox.join("a.txt");
        std::fs::write(&a, b"a").unwrap();

        // No preview stored.
        let err = execute_previewed(&f.state, &f.db, &f.cfg, "missing", |_| panic!("ran"))
            .await
            .unwrap_err();
        assert_eq!(err, PREVIEW_REQUIRED);

        // A preview stored under another id.
        let plan = Plan {
            ops: vec![planned_move(&a, &f.out.join("a.txt"))],
            skipped: vec![],
            reports: vec![],
        };
        f.state.store_preview("p-1".to_string(), plan, vec![]);
        let err = execute_previewed(&f.state, &f.db, &f.cfg, "p-2", |_| panic!("ran"))
            .await
            .unwrap_err();
        assert_eq!(err, PREVIEW_REQUIRED);

        assert_eq!(std::fs::read(&a).unwrap(), b"a");
        assert!(!f.out.join("a.txt").exists());
        assert!(f.db.journal().list_batches(10).await.unwrap().is_empty());
        assert!(f.db.get_recent_operations(10).await.unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn execute_runs_only_the_previewed_ops_and_consumes_the_plan() {
        let f = fixture().await;
        let a = f.inbox.join("a.txt");
        std::fs::write(&a, b"a").unwrap();
        let plan = Plan {
            ops: vec![planned_move(&a, &f.out.join("a.txt"))],
            skipped: vec![],
            reports: vec![],
        };
        f.state.store_preview("p-1".to_string(), plan, vec![]);

        // Appears after the preview; the same rule would match it.
        let late = f.inbox.join("late.txt");
        std::fs::write(&late, b"late").unwrap();

        let mut seen = 0;
        let report = execute_previewed(&f.state, &f.db, &f.cfg, "p-1", |p| seen = p.ops.len())
            .await
            .unwrap();
        assert_eq!(seen, 1);
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.done_count(), 1);
        assert!(!a.exists());
        assert_eq!(std::fs::read(f.out.join("a.txt")).unwrap(), b"a");
        assert_eq!(std::fs::read(&late).unwrap(), b"late");
        assert!(!f.out.join("late.txt").exists());
        assert_eq!(f.db.get_recent_operations(10).await.unwrap().len(), 1);

        // The plan is used up.
        let err = execute_previewed(&f.state, &f.db, &f.cfg, "p-1", |_| panic!("ran"))
            .await
            .unwrap_err();
        assert_eq!(err, PREVIEW_REQUIRED);
        assert_eq!(f.db.journal().list_batches(10).await.unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn execute_approves_the_previewed_rules_at_their_version() {
        use crate::commands::settings::{load_app_settings, load_approvals};
        let f = fixture().await;
        let a = f.inbox.join("a.txt");
        std::fs::write(&a, b"a").unwrap();
        let mut op = planned_move(&a, &f.out.join("a.txt"));
        op.rule_id = uuid::Uuid::from_u128(7);
        let plan = Plan {
            ops: vec![op],
            skipped: vec![],
            reports: vec![],
        };

        // A refused execute approves nothing.
        f.state
            .store_preview("p-1".to_string(), plan, vec![(uuid::Uuid::from_u128(7), 2)]);
        execute_previewed(&f.state, &f.db, &f.cfg, "p-2", |_| {})
            .await
            .unwrap_err();
        assert!(load_approvals(&f.cfg).0.is_empty());
        assert!(!load_app_settings(&f.cfg).first_preview_approved);

        execute_previewed(&f.state, &f.db, &f.cfg, "p-1", |_| {})
            .await
            .unwrap();
        let approvals = load_approvals(&f.cfg);
        assert_eq!(approvals.0.get(&uuid::Uuid::from_u128(7)), Some(&2));
        assert!(load_app_settings(&f.cfg).first_preview_approved);

        let mut r = rule(7, "r", 2);
        assert!(approvals.allows(&r));
        r.version = 3; // edited after the preview
        assert!(!approvals.allows(&r));
    }

    fn rule(n: u128, name: &str, version: i32) -> Rule {
        serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::from_u128(n),
            "name": name,
            "enabled": true,
            "conditions": [],
            "actions": [{ "type": "report" }],
            "version": version,
        }))
        .unwrap()
    }

    #[test]
    fn rules_in_plan_has_rules_with_ops_reports_or_skips() {
        let op_rule = rule(1, "moves", 3);
        let report_rule = rule(2, "reports", 1);
        let skip_rule = rule(3, "skips", 5);
        let unused = rule(4, "unused", 1);
        let mut disabled = rule(5, "skips", 1);
        disabled.enabled = false;
        let plan = Plan {
            ops: vec![PlannedOp {
                source: PathBuf::from("in/a"),
                dest: PathBuf::from("out/a"),
                kind: OpKind::Move,
                rule_id: op_rule.id,
                rule_name: op_rule.name.clone(),
                size: 1,
                mtime: std::time::SystemTime::UNIX_EPOCH,
            }],
            skipped: vec![
                valet_core::planner::Skipped {
                    path: PathBuf::from("in/b"),
                    reason: valet_core::planner::SkipReason::Conflict,
                    rule_name: Some("skips".to_string()),
                },
                valet_core::planner::Skipped {
                    path: PathBuf::from("in/.git"),
                    reason: valet_core::planner::SkipReason::Excluded,
                    rule_name: None,
                },
            ],
            reports: vec![valet_core::planner::ReportEntry {
                path: PathBuf::from("in/c"),
                rule_id: report_rule.id,
                rule_name: report_rule.name.clone(),
                size: 1,
                group: None,
            }],
        };
        let rules = [op_rule, report_rule, skip_rule, unused, disabled];
        assert_eq!(
            rules_in_plan(&plan, &rules),
            vec![
                (uuid::Uuid::from_u128(1), 3),
                (uuid::Uuid::from_u128(2), 1),
                (uuid::Uuid::from_u128(3), 5),
            ]
        );
    }

    fn result(name: &str, kind: OpKind, error: Option<OpError>) -> OpResult {
        OpResult {
            op_id: 1,
            kind,
            source: PathBuf::from("in").join(name),
            dest: PathBuf::from("out").join(name),
            rule_name: "r".to_string(),
            size: 4,
            error,
        }
    }

    #[test]
    fn execute_response_keeps_frontend_fields_and_adds_batch_id() {
        let report = BatchReport {
            batch_id: "b-1".to_string(),
            results: vec![
                result("a.txt", OpKind::Move, None),
                result(
                    "b.txt",
                    OpKind::Copy,
                    Some(OpError {
                        code: FailCode::Locked,
                        detail: None,
                    }),
                ),
            ],
        };
        let v = serde_json::to_value(execute_response(&report)).unwrap();
        assert_eq!(v["executed_count"], 1);
        assert_eq!(v["success"], false);
        assert_eq!(v["batch_id"], "b-1");
        let f = &v["failed_operations"][0];
        // Same construction as `result`: `join` uses `\` on Windows.
        assert_eq!(
            f["source_path"],
            PathBuf::from("in").join("b.txt").to_string_lossy().as_ref()
        );
        assert_eq!(
            f["destination_path"],
            PathBuf::from("out")
                .join("b.txt")
                .to_string_lossy()
                .as_ref()
        );
        assert_eq!(f["rule_name"], "r");
        assert_eq!(f["error_message"], "Locked");
        assert_eq!(v["failed_operations"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn execute_response_all_done_is_success() {
        let report = BatchReport {
            batch_id: "b-2".to_string(),
            results: vec![result("a.txt", OpKind::Move, None)],
        };
        let r = execute_response(&report);
        assert!(r.success && r.failed_operations.is_empty());
        assert_eq!(r.executed_count, 1);
    }

    #[test]
    fn dry_run_json_keeps_frontend_fields_and_adds_skipped() {
        let plan = Plan {
            ops: vec![PlannedOp {
                source: PathBuf::from("in/a.pdf"),
                dest: PathBuf::from("in/Out/a.pdf"),
                kind: OpKind::Copy,
                rule_id: uuid::Uuid::nil(),
                rule_name: "r".to_string(),
                size: 1,
                mtime: std::time::SystemTime::UNIX_EPOCH,
            }],
            skipped: vec![valet_core::planner::Skipped {
                path: PathBuf::from("in/.git"),
                reason: valet_core::planner::SkipReason::Excluded,
                rule_name: None,
            }],
            reports: vec![],
        };
        let v = dry_run_json(&plan, "p-1");
        assert_eq!(v["total_files"], 1);
        assert_eq!(v["total_rules_applied"], 1);
        let r = &v["results"][0];
        assert_eq!(
            r["source_path"],
            PathBuf::from("in/a.pdf").to_string_lossy().as_ref()
        );
        assert_eq!(
            r["destination_path"],
            PathBuf::from("in/Out/a.pdf").to_string_lossy().as_ref()
        );
        assert_eq!(r["rule_name"], "r");
        assert_eq!(r["operation"], "copy");
        let k = &v["skipped"][0];
        assert_eq!(
            k["path"],
            PathBuf::from("in/.git").to_string_lossy().as_ref()
        );
        assert_eq!(k["reason"], "excluded");
        assert!(k["rule_name"].is_null());
        assert_eq!(v["plan_id"], "p-1");
        assert_eq!(v["reports"], serde_json::json!([]));
    }

    #[test]
    fn dry_run_json_has_trash_ops_and_reports() {
        let plan = Plan {
            ops: vec![PlannedOp {
                source: PathBuf::from("in/setup.exe"),
                dest: PathBuf::new(),
                kind: OpKind::Trash,
                rule_id: uuid::Uuid::nil(),
                rule_name: "old".to_string(),
                size: 3,
                mtime: std::time::SystemTime::UNIX_EPOCH,
            }],
            skipped: vec![],
            reports: vec![valet_core::planner::ReportEntry {
                path: PathBuf::from("in/a.iso"),
                rule_id: uuid::Uuid::from_u128(5),
                rule_name: "big".to_string(),
                size: 9,
                group: Some("abc123".to_string()),
            }],
        };
        let v = dry_run_json(&plan, "p-2");
        let r = &v["results"][0];
        assert_eq!(r["operation"], "trash");
        assert_eq!(r["destination_path"], "");
        let rep = &v["reports"][0];
        assert_eq!(
            rep["path"],
            PathBuf::from("in/a.iso").to_string_lossy().as_ref()
        );
        assert_eq!(rep["rule_id"], uuid::Uuid::from_u128(5).to_string());
        assert_eq!(rep["rule_name"], "big");
        assert_eq!(rep["size"], 9);
        assert_eq!(rep["group"], "abc123");
    }
}
