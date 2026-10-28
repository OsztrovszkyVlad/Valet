//! Operation commands: dry run, execute (sequential and bulk), statistics, history.

use std::path::Path;

use tauri::{Emitter, Manager};
use valet_core::{model::DryRunAction, storage::Db};

use crate::commands::platform::{send_notification, NotificationLevel};
use crate::commands::settings::{get_app_settings, Config};
use crate::state::AppState;

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
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;
  
  // Get configured paths from config
  let app_cfg_dir = app.path().app_config_dir()
    .map_err(|e| format!("Failed to get config directory: {}", e))?;
  let cfg_path = app_cfg_dir.join("config.json");
  
  let paths = if cfg_path.exists() {
    let contents = std::fs::read_to_string(&cfg_path)
      .map_err(|e| format!("Failed to read config file: {}", e))?;
    let config: Config = serde_json::from_str(&contents)
      .map_err(|e| format!("Failed to parse config JSON: {}", e))?;
    
    config.inbox_paths.into_iter()
      .map(|p| std::path::PathBuf::from(p))
      .filter(|p| p.exists())
      .collect()
  } else {
    vec![]
  };
  
  if paths.is_empty() {
    return Err("No valid configured paths found. Please set up your inbox paths in Preferences.".to_string());
  }
  
  let plan = valet_core::engine::dry_run_for_paths(&paths, &db).await
    .map_err(|e| format!("Dry run failed: {}", e))?;
  
  log::info!("Dry-run completed: {} action(s) proposed across {} path(s)", plan.actions.len(), paths.len());
  
  // Convert to a format that matches the frontend interface
  let results: Vec<serde_json::Value> = plan.actions.into_iter().map(|action| {
    serde_json::json!({
      "source_path": action.file_path,
      "destination_path": match &action.op {
        valet_core::model::Op::MoveTo { path } => path.clone(),
        valet_core::model::Op::CopyTo { path } => path.clone(),
        _ => "N/A".to_string(),
      },
      "rule_name": action.rule_name,
      "operation": match &action.op {
        valet_core::model::Op::MoveTo { .. } => "move",
        valet_core::model::Op::CopyTo { .. } => "copy",
        _ => "unknown",
      }
    })
  }).collect();
  
  Ok(serde_json::json!({
    "results": results,
    "total_files": results.len(),
    "total_rules_applied": results.len()
  }))
}

#[tauri::command]
pub(crate) async fn execute_operations_bulk(app: tauri::AppHandle, max_concurrent: Option<usize>) -> Result<ExecuteResponse, String> {
  log::info!("Executing bulk file operations");
  
  let start_time = std::time::Instant::now();
  let state = app.state::<AppState>();
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;

  // Load settings for notifications and concurrency
  let settings = get_app_settings(app.clone()).await.unwrap_or_default();
  let concurrent_ops = max_concurrent.unwrap_or(settings.max_concurrent_operations as usize);
  
  // Get configured paths from config
  let app_cfg_dir = app.path().app_config_dir()
    .map_err(|e| format!("Failed to get config directory: {}", e))?;
  let cfg_path = app_cfg_dir.join("config.json");
  
  let paths = if cfg_path.exists() {
    let contents = std::fs::read_to_string(&cfg_path)
      .map_err(|e| format!("Failed to read config file: {}", e))?;
    let config: Config = serde_json::from_str(&contents)
      .map_err(|e| format!("Failed to parse config JSON: {}", e))?;
    
    config.inbox_paths.into_iter()
      .map(|p| std::path::PathBuf::from(p))
      .filter(|p| p.exists())
      .collect()
  } else {
    vec![]
  };
  
  if paths.is_empty() {
    return Err("No valid configured paths found. Please set up your inbox paths in Preferences.".to_string());
  }
  
  // Get the dry run plan
  let plan = valet_core::engine::dry_run_for_paths(&paths, &db).await
    .map_err(|e| format!("Failed to generate execution plan: {}", e))?;
  
  log::info!("Executing {} file operation(s) with {} concurrent workers", plan.actions.len(), concurrent_ops);

  // Notify user of starting operations
  if settings.notifications_enabled && plan.actions.len() > 0 {
    let _ = send_notification(
      "Valet File Manager".to_string(),
      format!("Starting bulk organization of {} files", plan.actions.len()),
      app.clone()
    ).await;
  }
  
  use tokio::sync::{Semaphore, Mutex};
  use std::sync::Arc;
  
  let semaphore = Arc::new(Semaphore::new(concurrent_ops));
  let executed_count = Arc::new(Mutex::new(0usize));
  let failed_operations = Arc::new(Mutex::new(Vec::new()));
  let total_bytes_processed = Arc::new(Mutex::new(0u64));
  
  let total_operations = plan.actions.len();
  let mut handles = Vec::new();
  
  for (index, action) in plan.actions.into_iter().enumerate() {
    let permit = semaphore.clone().acquire_owned().await
      .map_err(|e| format!("Failed to acquire semaphore: {}", e))?;
    
    let executed_count = executed_count.clone();
    let failed_operations = failed_operations.clone();
    let total_bytes_processed = total_bytes_processed.clone();
    let app_handle = app.clone();
    let db_path = state._db_path.clone();
    
    let handle = tokio::spawn(async move {
      let _permit = permit; // Keep permit alive
      
      // Send progress update
      let progress = ProgressUpdate {
        current: index + 1,
        total: total_operations,
        current_file: action.file_path.clone(),
        percentage: ((index + 1) as f32 / total_operations as f32) * 100.0,
      };
      
      if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.emit("operation-progress", &progress);
      }
      
      let operation_type = match &action.op {
        valet_core::model::Op::MoveTo { .. } => "move",
        valet_core::model::Op::CopyTo { .. } => "copy",
        _ => "other",
      };
      
      let destination_path = match &action.op {
        valet_core::model::Op::MoveTo { path } => path.clone(),
        valet_core::model::Op::CopyTo { path } => path.clone(),
        _ => "N/A".to_string(),
      };
      
      // Get file size for metrics
      if let Ok(metadata) = std::fs::metadata(&action.file_path) {
        let mut bytes = total_bytes_processed.lock().await;
        *bytes += metadata.len();
      }
      
      match execute_single_operation(&action).await {
        Ok(_) => {
          let mut count = executed_count.lock().await;
          *count += 1;
          log::info!("✅ Successfully executed: {} -> {:?}", action.file_path, action.op);
          
          // Record successful operation
          if let Ok(db) = valet_core::storage::Db::connect(&db_path).await {
            if let Err(e) = db.record_operation(
              &action.file_path,
              &destination_path,
              operation_type,
              &action.rule_name,
              "success",
              None
            ).await {
              log::warn!("Failed to record successful operation: {}", e);
            }
          }
        }
        Err(e) => {
          let error = ExecuteError {
            source_path: action.file_path.clone(),
            destination_path: destination_path.clone(),
            rule_name: action.rule_name.clone(),
            error_message: e.clone(),
          };
          
          let mut errors = failed_operations.lock().await;
          errors.push(error);
          log::error!("❌ Failed to execute operation: {} -> {:?}, Error: {}", action.file_path, action.op, e);
          
          // Record failed operation
          if let Ok(db) = valet_core::storage::Db::connect(&db_path).await {
            if let Err(record_err) = db.record_operation(
              &action.file_path,
              &destination_path,
              operation_type,
              &action.rule_name,
              "failed",
              Some(&e)
            ).await {
              log::warn!("Failed to record failed operation: {}", record_err);
            }
          }
        }
      }
    });
    
    handles.push(handle);
  }
  
  // Wait for all operations to complete
  for handle in handles {
    let _ = handle.await;
  }
  
  let final_executed_count = *executed_count.lock().await;
  let final_failed_operations = failed_operations.lock().await.clone();
  let final_bytes_processed = *total_bytes_processed.lock().await;
  
  let success = final_failed_operations.is_empty();
  let total_time = start_time.elapsed();
  let total_time_ms = total_time.as_millis();
  
  // Calculate performance metrics
  let files_per_second = if total_time.as_secs_f32() > 0.0 {
    final_executed_count as f32 / total_time.as_secs_f32()
  } else {
    0.0
  };
  
  let performance_metrics = PerformanceMetrics {
    total_files_processed: final_executed_count,
    total_time_ms,
    files_per_second,
    bytes_processed: final_bytes_processed,
    errors_count: final_failed_operations.len(),
  };
  
  log::info!("Bulk operation completed: {}/{} successful, {} failed in {}ms", 
    final_executed_count, final_executed_count + final_failed_operations.len(), final_failed_operations.len(), total_time_ms);
  log::info!("Performance: {:.2} files/sec, {} bytes processed", files_per_second, final_bytes_processed);

  // Send performance metrics to frontend
  if let Some(window) = app.get_webview_window("main") {
    let _ = window.emit("operation-complete", &performance_metrics);
  }

  // Send completion notification
  if settings.notifications_enabled {
    let (title, message, _level) = if success {
      ("Bulk Organization Complete", 
       &format!("Successfully organized {} files in {:.1}s ({:.1} files/sec)", final_executed_count, total_time.as_secs_f32(), files_per_second), 
       NotificationLevel::Success)
    } else {
      ("Bulk Organization Completed with Issues", 
       &format!("{} files organized, {} failed in {:.1}s", final_executed_count, final_failed_operations.len(), total_time.as_secs_f32()), 
       NotificationLevel::Warning)
    };
    
    let _ = send_notification(
      title.to_string(),
      message.to_string(),
      app.clone()
    ).await;
  }
  
  Ok(ExecuteResponse {
    executed_count: final_executed_count,
    failed_operations: final_failed_operations,
    success,
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
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;

  // Load settings for notifications
  let settings = get_app_settings(app.clone()).await.unwrap_or_default();
  
  // Get configured paths from config
  let app_cfg_dir = app.path().app_config_dir()
    .map_err(|e| format!("Failed to get config directory: {}", e))?;
  let cfg_path = app_cfg_dir.join("config.json");
  
  let paths = if cfg_path.exists() {
    let contents = std::fs::read_to_string(&cfg_path)
      .map_err(|e| format!("Failed to read config file: {}", e))?;
    let config: Config = serde_json::from_str(&contents)
      .map_err(|e| format!("Failed to parse config JSON: {}", e))?;
    
    config.inbox_paths.into_iter()
      .map(|p| std::path::PathBuf::from(p))
      .filter(|p| p.exists())
      .collect()
  } else {
    vec![]
  };
  
  if paths.is_empty() {
    return Err("No valid configured paths found. Please set up your inbox paths in Preferences.".to_string());
  }
  
  // First, get the dry run plan for all configured paths
  let plan = valet_core::engine::dry_run_for_paths(&paths, &db).await
    .map_err(|e| format!("Failed to generate execution plan: {}", e))?;
  
  log::info!("Executing {} file operation(s) across {} path(s)", plan.actions.len(), paths.len());

  // Notify user of starting operations
  if settings.notifications_enabled && plan.actions.len() > 0 {
    let _ = send_notification(
      "Valet File Manager".to_string(),
      format!("Starting organization of {} files", plan.actions.len()),
      app.clone()
    ).await;
  }
  
  let mut executed_count = 0;
  let mut failed_operations = Vec::new();
  let mut total_bytes_processed = 0u64;
  let total_operations = plan.actions.len();
  
  // Process operations with progress tracking
  for (index, action) in plan.actions.iter().enumerate() {
    // Send progress update
    let progress = ProgressUpdate {
      current: index + 1,
      total: total_operations,
      current_file: action.file_path.clone(),
      percentage: ((index + 1) as f32 / total_operations as f32) * 100.0,
    };
    
    if let Some(window) = app.get_webview_window("main") {
      let _ = window.emit("operation-progress", &progress);
    }
    
    let operation_type = match &action.op {
      valet_core::model::Op::MoveTo { .. } => "move",
      valet_core::model::Op::CopyTo { .. } => "copy",
      _ => "other",
    };
    
    let destination_path = match &action.op {
      valet_core::model::Op::MoveTo { path } => path.clone(),
      valet_core::model::Op::CopyTo { path } => path.clone(),
      _ => "N/A".to_string(),
    };
    
    // Get file size for metrics
    if let Ok(metadata) = std::fs::metadata(&action.file_path) {
      total_bytes_processed += metadata.len();
    }
    
    match execute_single_operation(&action).await {
      Ok(_) => {
        executed_count += 1;
        log::info!("✅ Successfully executed: {} -> {:?}", action.file_path, action.op);
        
        // Record successful operation
        if let Err(e) = db.record_operation(
          &action.file_path,
          &destination_path,
          operation_type,
          &action.rule_name,
          "success",
          None
        ).await {
          log::warn!("Failed to record successful operation: {}", e);
        }
      }
      Err(e) => {
        let error_msg = e.clone();
        let error = ExecuteError {
          source_path: action.file_path.clone(),
          destination_path: destination_path.clone(),
          rule_name: action.rule_name.clone(),
          error_message: e.clone(),
        };
        failed_operations.push(error);
        log::error!("❌ Failed to execute operation: {} -> {:?}, Error: {}", action.file_path, action.op, error_msg);
        
        // Record failed operation
        if let Err(record_err) = db.record_operation(
          &action.file_path,
          &destination_path,
          operation_type,
          &action.rule_name,
          "failed",
          Some(&e)
        ).await {
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
  
  log::info!("Operation completed: {}/{} successful, {} failed in {}ms", 
    executed_count, executed_count + failed_operations.len(), failed_operations.len(), total_time_ms);
  log::info!("Performance: {:.2} files/sec, {} bytes processed", files_per_second, total_bytes_processed);

  // Send performance metrics to frontend
  if let Some(window) = app.get_webview_window("main") {
    let _ = window.emit("operation-complete", &performance_metrics);
  }

  // Send completion notification
  if settings.notifications_enabled {
    let (title, message, _level) = if success {
      ("Organization Complete", 
       &format!("Successfully organized {} files in {:.1}s", executed_count, total_time.as_secs_f32()), 
       NotificationLevel::Success)
    } else {
      ("Organization Completed with Issues", 
       &format!("{} files organized, {} failed in {:.1}s", executed_count, failed_operations.len(), total_time.as_secs_f32()), 
       NotificationLevel::Warning)
    };
    
    let _ = send_notification(
      title.to_string(),
      message.to_string(),
      app.clone()
    ).await;
  }
  
  Ok(ExecuteResponse {
    executed_count,
    failed_operations,
    success,
  })
}

async fn execute_single_operation(action: &DryRunAction) -> Result<(), String> {
  execute_single_operation_with_retry(action, &RetryConfig::default()).await
}

async fn execute_single_operation_with_retry(action: &DryRunAction, retry_config: &RetryConfig) -> Result<(), String> {
  use std::path::Path;
  
  let source_path = Path::new(&action.file_path);
  
  // Validate source file exists and is accessible
  if !source_path.exists() {
    return Err(format!("Source file does not exist: {}", action.file_path));
  }
  
  if let Ok(metadata) = source_path.metadata() {
    if metadata.len() == 0 {
      log::warn!("Warning: Attempting to move zero-byte file: {}", action.file_path);
    }
  }
  
  let mut last_error = String::new();
  let mut attempt = 0;
  
  while attempt <= retry_config.max_retries {
    match execute_file_operation_internal(action, source_path).await {
      Ok(_) => {
        if attempt > 0 {
          log::info!("✅ Operation succeeded after {} retries: {}", attempt, action.file_path);
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
          
          log::warn!("⚠️ Operation failed (attempt {}/{}): {}. Retrying in {}ms...", 
            attempt, retry_config.max_retries + 1, e, delay);
          
          tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        } else {
          log::error!("❌ Operation failed after {} attempts: {}", attempt, e);
        }
      }
    }
  }
  
  Err(format!("Operation failed after {} attempts. Last error: {}", retry_config.max_retries + 1, last_error))
}

async fn execute_file_operation_internal(action: &DryRunAction, source_path: &Path) -> Result<(), String> {
  use std::fs;
  
  match &action.op {
    valet_core::model::Op::MoveTo { path } => {
      let dest_path = Path::new(path);
      
      // Check if destination already exists
      if dest_path.exists() {
        return Err(format!("Destination file already exists: {}", path));
      }
      
      // Ensure destination directory exists
      if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
          .map_err(|e| format!("Failed to create destination directory '{}': {}", parent.display(), e))?;
      }
      
      // Phase 0: removed permanent deletion (invariant 1).
      
      // Move the file
      fs::rename(source_path, dest_path)
        .map_err(|e| format!("Failed to move file from '{}' to '{}': {}", source_path.display(), dest_path.display(), e))?;
      
      Ok(())
    }
    valet_core::model::Op::CopyTo { path } => {
      let dest_path = Path::new(path);
      
      // Check if destination already exists
      if dest_path.exists() {
        return Err(format!("Destination file already exists: {}", path));
      }
      
      // Ensure destination directory exists
      if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
          .map_err(|e| format!("Failed to create destination directory '{}': {}", parent.display(), e))?;
      }
      
      // Phase 0: removed permanent deletion (invariant 1).
      
      // Copy the file
      fs::copy(source_path, dest_path)
        .map_err(|e| format!("Failed to copy file from '{}' to '{}': {}", source_path.display(), dest_path.display(), e))?;
      
      Ok(())
    }
    _ => {
      Err("Unsupported operation type".to_string())
    }
  }
}

/*
// TODO: Implement rollback functions when database methods are available
#[derive(serde::Serialize, serde::Deserialize)]
struct RollbackOperation {
  operation_id: String,
  original_path: String,
  moved_to_path: String,
  operation_type: String,
  timestamp: String,
}

#[tauri::command]
async fn rollback_operation(operation_id: String, app: tauri::AppHandle) -> Result<(), String> {
  // Implementation commented out until database methods are implemented
  Err("Rollback functionality not yet implemented".to_string())
}

#[tauri::command] 
async fn get_rollbackable_operations(app: tauri::AppHandle, limit: Option<usize>) -> Result<Vec<RollbackOperation>, String> {
  // Implementation commented out until database methods are implemented  
  Err("Rollback functionality not yet implemented".to_string())
}
*/

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct StatsResponse {
  stats: valet_core::model::OperationStats,
  recent_operations: Vec<valet_core::model::RecentOperation>,
}

#[tauri::command]
pub(crate) async fn get_statistics(time_range: String, app: tauri::AppHandle) -> Result<StatsResponse, String> {
  log::info!("Getting statistics for time range: {}", time_range);
  
  let state = app.state::<AppState>();
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;
  
  let days_back = match time_range.as_str() {
    "1d" => Some(1),
    "7d" => Some(7),
    "30d" => Some(30),
    "90d" => Some(90),
    "all" => None,
    _ => Some(7), // default to 7 days
  };
  
  let stats = db.get_operation_statistics(days_back).await
    .map_err(|e| format!("Failed to get statistics: {}", e))?;
  
  let recent_operations = db.get_recent_operations(50).await
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
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;
  
  db.clear_operation_history().await
    .map_err(|e| format!("Failed to clear operation history: {}", e))?;
  
  log::info!("Operation history cleared successfully");
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use valet_core::model::Op;

  // No `tempfile` dev-dependency in this crate yet, so each test uses a fresh
  // directory under the OS temp dir. It is not removed (invariant 1).
  fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("valet-app-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
  }

  fn run(op: fn(String) -> Op) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let dir = scratch_dir();
    let src = dir.join("a.txt");
    std::fs::write(&src, b"data").unwrap();
    let dest_dir = dir.join("dest");
    std::fs::create_dir_all(&dest_dir).unwrap();
    let user_file = dest_dir.join(".valet_test");
    std::fs::write(&user_file, b"user data").unwrap();
    let dest = dest_dir.join("a.txt");
    let action = DryRunAction {
      file_path: src.to_string_lossy().to_string(),
      op: op(dest.to_string_lossy().to_string()),
      rule_id: uuid::Uuid::nil(),
      rule_name: "test".to_string(),
    };

    tauri::async_runtime::block_on(execute_file_operation_internal(&action, &src)).unwrap();

    assert_eq!(std::fs::read(&user_file).unwrap(), b"user data");
    assert_eq!(std::fs::read(&dest).unwrap(), b"data");
    (src, dest, user_file)
  }

  #[test]
  fn move_leaves_existing_valet_test_file_untouched() {
    let (src, _dest, _user_file) = run(|path| Op::MoveTo { path });
    assert!(!src.exists());
  }

  #[test]
  fn copy_leaves_existing_valet_test_file_untouched() {
    let (src, _dest, _user_file) = run(|path| Op::CopyTo { path });
    assert_eq!(std::fs::read(&src).unwrap(), b"data");
  }
}
