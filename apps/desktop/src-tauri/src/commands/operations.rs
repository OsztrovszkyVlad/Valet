//! Operation commands: dry run, execute, statistics, history.

use std::path::PathBuf;

use tauri::{Emitter, Manager};
use valet_core::{
    executor::{self, BatchReport, UndoReport},
    journal::{BatchInfo, OpRecord},
    planner::{self, OpKind, Plan, PlanContext},
    platform::Platform,
    storage::Db,
};

use crate::commands::platform::{send_notification, NotificationLevel};
use crate::commands::settings::{get_app_settings, Config};
use crate::state::{config_dir, AppState};

/// Plans moves and copies for `paths` with the rules stored in `db`.
pub(crate) async fn plan_for_paths(
    paths: &[PathBuf],
    db: &Db,
    platform: &dyn Platform,
) -> Result<Plan, String> {
    let rules = db
        .list_rules()
        .await
        .map_err(|e| format!("Failed to load rules: {}", e))?;
    Ok(planner::plan(
        paths,
        &rules,
        platform,
        &PlanContext::from_system(),
    ))
}

fn op_type(kind: OpKind) -> &'static str {
    match kind {
        OpKind::Move => "move",
        OpKind::Copy => "copy",
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

    // Get configured paths from config
    let app_cfg_dir = config_dir(&app)?;
    let cfg_path = app_cfg_dir.join("config.json");

    let paths = if cfg_path.exists() {
        let contents = std::fs::read_to_string(&cfg_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        let config: Config = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse config JSON: {}", e))?;

        config
            .inbox_paths
            .into_iter()
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
            .collect()
    } else {
        vec![]
    };

    if paths.is_empty() {
        return Err(
            "No valid configured paths found. Please set up your inbox paths in Preferences."
                .to_string(),
        );
    }

    let plan = plan_for_paths(&paths, &db, state.platform.as_ref())
        .await
        .map_err(|e| format!("Dry run failed: {}", e))?;

    log::info!(
        "Dry-run completed: {} operation(s) proposed, {} skipped, across {} path(s)",
        plan.ops.len(),
        plan.skipped.len(),
        paths.len()
    );

    Ok(dry_run_json(&plan))
}

/// The shape `DryRunView.tsx` reads, plus `skipped` (path, reason, rule_name).
fn dry_run_json(plan: &Plan) -> serde_json::Value {
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

    serde_json::json!({
      "total_files": results.len(),
      "total_rules_applied": results.len(),
      "results": results,
      "skipped": skipped,
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

#[tauri::command]
pub(crate) async fn execute_operations(app: tauri::AppHandle) -> Result<ExecuteResponse, String> {
    log::info!("Executing file operations on configured paths");

    let start_time = std::time::Instant::now();
    let state = app.state::<AppState>();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    // Load settings for notifications
    let settings = get_app_settings(app.clone()).await.unwrap_or_default();

    // Get configured paths from config
    let app_cfg_dir = config_dir(&app)?;
    let cfg_path = app_cfg_dir.join("config.json");

    let paths = if cfg_path.exists() {
        let contents = std::fs::read_to_string(&cfg_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        let config: Config = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse config JSON: {}", e))?;

        config
            .inbox_paths
            .into_iter()
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
            .collect()
    } else {
        vec![]
    };

    if paths.is_empty() {
        return Err(
            "No valid configured paths found. Please set up your inbox paths in Preferences."
                .to_string(),
        );
    }

    // One batch at a time: plan and execute under the same lock.
    let _file_ops = state.file_ops.lock().await;

    let plan = plan_for_paths(&paths, &db, state.platform.as_ref())
        .await
        .map_err(|e| format!("Failed to generate execution plan: {}", e))?;

    log::info!(
        "Executing {} file operation(s) across {} path(s)",
        plan.ops.len(),
        paths.len()
    );

    // Notify user of starting operations
    if settings.notifications_enabled && !plan.ops.is_empty() {
        let _ = send_notification(
            "Valet File Manager".to_string(),
            format!("Starting organization of {} files", plan.ops.len()),
            app.clone(),
        )
        .await;
    }

    let report = executor::execute(&plan.ops, &db.journal(), state.platform.as_ref())
        .await
        .map_err(|e| format!("Operation journal could not be written: {:#}", e))?;

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

    // Send completion notification
    if settings.notifications_enabled {
        let (title, message, _level) = if response.success {
            (
                "Organization Complete",
                &format!(
                    "Successfully organized {} files in {:.1}s",
                    executed_count,
                    total_time.as_secs_f32()
                ),
                NotificationLevel::Success,
            )
        } else {
            (
                "Organization Completed with Issues",
                &format!(
                    "{} files organized, {} failed in {:.1}s",
                    executed_count,
                    failed_count,
                    total_time.as_secs_f32()
                ),
                NotificationLevel::Warning,
            )
        };

        let _ = send_notification(title.to_string(), message.to_string(), app.clone()).await;
    }

    Ok(response)
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
    use valet_core::planner::PlannedOp;

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
        assert_eq!(
            f["source_path"],
            PathBuf::from("in/b.txt").to_string_lossy().as_ref()
        );
        assert_eq!(
            f["destination_path"],
            PathBuf::from("out/b.txt").to_string_lossy().as_ref()
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
        };
        let v = dry_run_json(&plan);
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
    }
}
