//! Operation commands: dry run, execute, statistics, history.

use std::path::{Path, PathBuf};

use tauri::{Emitter, Manager};
use valet_core::{
    planner::{self, OpKind, Plan, PlanContext, PlannedOp},
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

fn op_type(op: &PlannedOp) -> &'static str {
    match op.kind {
        OpKind::Move => "move",
        OpKind::Copy => "copy",
    }
}

// Tauri command types
#[derive(serde::Serialize, Clone)]
struct ProgressUpdate {
    current: usize,
    total: usize,
    current_file: String,
    percentage: f32,
}

#[derive(serde::Serialize)]
struct PerformanceMetrics {
    total_files_processed: usize,
    total_time_ms: u128,
    files_per_second: f32,
    bytes_processed: u64,
    errors_count: usize,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct RetryConfig {
    max_retries: usize,
    retry_delay_ms: u64,
    exponential_backoff: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 1000,
            exponential_backoff: true,
        }
    }
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
              "operation": op_type(op),
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

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct ExecuteResponse {
    pub(crate) executed_count: usize,
    failed_operations: Vec<ExecuteError>,
    success: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ExecuteError {
    source_path: String,
    destination_path: String,
    rule_name: String,
    error_message: String,
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

    // First, get the dry run plan for all configured paths
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

    let mut executed_count = 0;
    let mut failed_operations = Vec::new();
    let mut total_bytes_processed = 0u64;
    let total_operations = plan.ops.len();

    // Process operations with progress tracking
    for (index, action) in plan.ops.iter().enumerate() {
        let source_path = action.source.to_string_lossy().to_string();
        // Send progress update
        let progress = ProgressUpdate {
            current: index + 1,
            total: total_operations,
            current_file: source_path.clone(),
            percentage: ((index + 1) as f32 / total_operations as f32) * 100.0,
        };

        if let Some(window) = app.get_webview_window("main") {
            let _ = window.emit("operation-progress", &progress);
        }

        let operation_type = op_type(action);

        let destination_path = action.dest.to_string_lossy().to_string();

        // Get file size for metrics
        if let Ok(metadata) = std::fs::metadata(&action.source) {
            total_bytes_processed += metadata.len();
        }

        match execute_single_operation(action).await {
            Ok(_) => {
                executed_count += 1;
                log::info!(
                    "✅ Successfully executed: {} -> {}",
                    source_path,
                    destination_path
                );

                // Record successful operation
                if let Err(e) = db
                    .record_operation(
                        &source_path,
                        &destination_path,
                        operation_type,
                        &action.rule_name,
                        "success",
                        None,
                    )
                    .await
                {
                    log::warn!("Failed to record successful operation: {}", e);
                }
            }
            Err(e) => {
                let error_msg = e.clone();
                let error = ExecuteError {
                    source_path: source_path.clone(),
                    destination_path: destination_path.clone(),
                    rule_name: action.rule_name.clone(),
                    error_message: e.clone(),
                };
                failed_operations.push(error);
                log::error!(
                    "❌ Failed to execute operation: {} -> {}, Error: {}",
                    source_path,
                    destination_path,
                    error_msg
                );

                // Record failed operation
                if let Err(record_err) = db
                    .record_operation(
                        &source_path,
                        &destination_path,
                        operation_type,
                        &action.rule_name,
                        "failed",
                        Some(&e),
                    )
                    .await
                {
                    log::warn!("Failed to record failed operation: {}", record_err);
                }
            }
        }
    }

    let success = failed_operations.is_empty();
    let total_time = start_time.elapsed();
    let total_time_ms = total_time.as_millis();

    // Calculate performance metrics
    let files_per_second = if total_time.as_secs_f32() > 0.0 {
        executed_count as f32 / total_time.as_secs_f32()
    } else {
        0.0
    };

    let performance_metrics = PerformanceMetrics {
        total_files_processed: executed_count,
        total_time_ms,
        files_per_second,
        bytes_processed: total_bytes_processed,
        errors_count: failed_operations.len(),
    };

    log::info!(
        "Operation completed: {}/{} successful, {} failed in {}ms",
        executed_count,
        executed_count + failed_operations.len(),
        failed_operations.len(),
        total_time_ms
    );
    log::info!(
        "Performance: {:.2} files/sec, {} bytes processed",
        files_per_second,
        total_bytes_processed
    );

    // Send performance metrics to frontend
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("operation-complete", &performance_metrics);
    }

    // Send completion notification
    if settings.notifications_enabled {
        let (title, message, _level) = if success {
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
                    failed_operations.len(),
                    total_time.as_secs_f32()
                ),
                NotificationLevel::Warning,
            )
        };

        let _ = send_notification(title.to_string(), message.to_string(), app.clone()).await;
    }

    Ok(ExecuteResponse {
        executed_count,
        failed_operations,
        success,
    })
}

async fn execute_single_operation(action: &PlannedOp) -> Result<(), String> {
    execute_single_operation_with_retry(action, &RetryConfig::default()).await
}

async fn execute_single_operation_with_retry(
    action: &PlannedOp,
    retry_config: &RetryConfig,
) -> Result<(), String> {
    let source_path = action.source.as_path();

    // Validate source file exists and is accessible
    if !source_path.exists() {
        return Err(format!(
            "Source file does not exist: {}",
            source_path.display()
        ));
    }

    if let Ok(metadata) = source_path.metadata() {
        if metadata.len() == 0 {
            log::warn!(
                "Warning: Attempting to move zero-byte file: {}",
                source_path.display()
            );
        }
    }

    let mut last_error = String::new();
    let mut attempt = 0;

    while attempt <= retry_config.max_retries {
        match execute_file_operation_internal(action, source_path).await {
            Ok(_) => {
                if attempt > 0 {
                    log::info!(
                        "✅ Operation succeeded after {} retries: {}",
                        attempt,
                        source_path.display()
                    );
                }
                return Ok(());
            }
            Err(e) => {
                last_error = e.clone();
                attempt += 1;

                if attempt <= retry_config.max_retries {
                    let delay = if retry_config.exponential_backoff {
                        retry_config.retry_delay_ms * (2_u64.pow(attempt as u32 - 1))
                    } else {
                        retry_config.retry_delay_ms
                    };

                    log::warn!(
                        "⚠️ Operation failed (attempt {}/{}): {}. Retrying in {}ms...",
                        attempt,
                        retry_config.max_retries + 1,
                        e,
                        delay
                    );

                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                } else {
                    log::error!("❌ Operation failed after {} attempts: {}", attempt, e);
                }
            }
        }
    }

    Err(format!(
        "Operation failed after {} attempts. Last error: {}",
        retry_config.max_retries + 1,
        last_error
    ))
}

async fn execute_file_operation_internal(
    action: &PlannedOp,
    source_path: &Path,
) -> Result<(), String> {
    use std::fs;

    let dest_path = action.dest.as_path();
    match action.kind {
        OpKind::Move => {
            // Check if destination already exists
            if dest_path.exists() {
                return Err(format!(
                    "Destination file already exists: {}",
                    dest_path.display()
                ));
            }

            // Ensure destination directory exists
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Failed to create destination directory '{}': {}",
                        parent.display(),
                        e
                    )
                })?;
            }

            // Phase 0: removed permanent deletion (invariant 1).

            // Move the file
            fs::rename(source_path, dest_path).map_err(|e| {
                format!(
                    "Failed to move file from '{}' to '{}': {}",
                    source_path.display(),
                    dest_path.display(),
                    e
                )
            })?;

            Ok(())
        }
        OpKind::Copy => {
            // Check if destination already exists
            if dest_path.exists() {
                return Err(format!(
                    "Destination file already exists: {}",
                    dest_path.display()
                ));
            }

            // Ensure destination directory exists
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Failed to create destination directory '{}': {}",
                        parent.display(),
                        e
                    )
                })?;
            }

            // Phase 0: removed permanent deletion (invariant 1).

            // Copy the file
            fs::copy(source_path, dest_path).map_err(|e| {
                format!(
                    "Failed to copy file from '{}' to '{}': {}",
                    source_path.display(),
                    dest_path.display(),
                    e
                )
            })?;

            Ok(())
        }
    }
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

    fn run(
        kind: OpKind,
    ) -> (
        tempfile::TempDir,
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let src = dir.join("a.txt");
        std::fs::write(&src, b"data").unwrap();
        let dest_dir = dir.join("dest");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let user_file = dest_dir.join(".valet_test");
        std::fs::write(&user_file, b"user data").unwrap();
        let dest = dest_dir.join("a.txt");
        let action = PlannedOp {
            source: src.clone(),
            dest: dest.clone(),
            kind,
            rule_id: uuid::Uuid::nil(),
            rule_name: "test".to_string(),
            size: 4,
            mtime: std::time::SystemTime::now(),
        };

        tauri::async_runtime::block_on(execute_file_operation_internal(&action, &src)).unwrap();

        assert_eq!(std::fs::read(&user_file).unwrap(), b"user data");
        assert_eq!(std::fs::read(&dest).unwrap(), b"data");
        (tmp, src, dest, user_file)
    }

    #[test]
    fn move_leaves_existing_valet_test_file_untouched() {
        let (_tmp, src, _dest, _user_file) = run(OpKind::Move);
        assert!(!src.exists());
    }

    #[test]
    fn copy_leaves_existing_valet_test_file_untouched() {
        let (_tmp, src, _dest, _user_file) = run(OpKind::Copy);
        assert_eq!(std::fs::read(&src).unwrap(), b"data");
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
