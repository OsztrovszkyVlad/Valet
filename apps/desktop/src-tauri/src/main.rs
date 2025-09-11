#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};
use std::path::PathBuf;

use tauri::{
  Manager,
  menu::{Menu, MenuItemBuilder, CheckMenuItemBuilder},
  tray::TrayIconBuilder,
};
use tauri_plugin_log::Builder;
use log::LevelFilter;
use valet_core::{config, storage::Db, rules::Rule, model::DryRunAction};

// Tauri command types
#[derive(serde::Serialize, serde::Deserialize)]
struct Config {
  inbox_paths: Vec<String>,
  pause_watchers: bool,
  #[serde(default = "default_quarantine_days")]
  quarantine_retention_days: u32,
}

fn default_quarantine_days() -> u32 { 30 }

// Tauri Commands
#[tauri::command]
async fn get_rules(app: tauri::AppHandle) -> Result<Vec<Rule>, String> {
  let state = app.state::<AppState>();
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;
  
  db.list_rules().await
    .map_err(|e| format!("Failed to list rules: {}", e))
}

#[tauri::command]
async fn upsert_rule(rule: Rule, app: tauri::AppHandle) -> Result<Rule, String> {
  log::info!("Upserting rule: {}", rule.name);
  
  let state = app.state::<AppState>();
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;
  
  // Validate rule
  if rule.name.trim().is_empty() {
    return Err("Rule name cannot be empty".to_string());
  }
  
  if rule.actions.is_empty() {
    return Err("Rule must have at least one action".to_string());
  }
  
  // Validate conditions
  for condition in &rule.conditions {
    match condition.r#type.as_str() {
      "nameMatches" => {
        if let Some(pattern) = condition.value.as_str() {
          regex::Regex::new(pattern)
            .map_err(|e| format!("Invalid regex pattern '{}': {}", pattern, e))?;
        } else {
          return Err("nameMatches condition requires a string value".to_string());
        }
      }
      "ext" | "pathContains" => {
        if condition.value.as_str().is_none() {
          return Err(format!("{} condition requires a string value", condition.r#type));
        }
      }
      "sizeGt" | "sizeLt" => {
        if condition.value.as_u64().is_none() {
          return Err(format!("{} condition requires a numeric value", condition.r#type));
        }
      }
      _ => {
        return Err(format!("Unknown condition type: {}", condition.r#type));
      }
    }
  }
  
  // Validate actions
  for action in &rule.actions {
    match action.r#type.as_str() {
      "moveTo" | "copyTo" => {
        if let Some(path) = action.params.get("path").and_then(|v| v.as_str()) {
          if path.trim().is_empty() {
            return Err(format!("{} action requires a non-empty path", action.r#type));
          }
        } else {
          return Err(format!("{} action requires a path parameter", action.r#type));
        }
      }
      "tag" => {
        if action.params.get("tags").and_then(|v| v.as_array()).is_none() {
          return Err("tag action requires a tags array parameter".to_string());
        }
      }
      "rename" => {
        if action.params.get("pattern").and_then(|v| v.as_str()).is_none() {
          return Err("rename action requires a pattern parameter".to_string());
        }
      }
      "quarantine" => {
        // No additional validation needed
      }
      _ => {
        return Err(format!("Unknown action type: {}", action.r#type));
      }
    }
  }
  
  db.upsert_rule(&rule).await
    .map_err(|e| format!("Failed to save rule: {}", e))?;
  
  log::info!("Rule '{}' saved successfully", rule.name);
  Ok(rule)
}

#[tauri::command]
async fn dry_run(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
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

#[derive(serde::Serialize, serde::Deserialize)]
struct ExecuteResponse {
  executed_count: usize,
  failed_operations: Vec<ExecuteError>,
  success: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ExecuteError {
  source_path: String,
  destination_path: String,
  rule_name: String,
  error_message: String,
}

#[tauri::command]
async fn execute_operations(app: tauri::AppHandle) -> Result<ExecuteResponse, String> {
  log::info!("Executing file operations on configured paths");
  
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
  
  // First, get the dry run plan for all configured paths
  let plan = valet_core::engine::dry_run_for_paths(&paths, &db).await
    .map_err(|e| format!("Failed to generate execution plan: {}", e))?;
  
  log::info!("Executing {} file operation(s) across {} path(s)", plan.actions.len(), paths.len());
  
  let mut executed_count = 0;
  let mut failed_operations = Vec::new();
  
  for action in plan.actions {
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
  log::info!("Operation completed: {}/{} successful, {} failed", 
    executed_count, executed_count + failed_operations.len(), failed_operations.len());
  
  Ok(ExecuteResponse {
    executed_count,
    failed_operations,
    success,
  })
}

async fn execute_single_operation(action: &DryRunAction) -> Result<(), String> {
  use std::fs;
  use std::path::Path;
  
  let source_path = Path::new(&action.file_path);
  
  match &action.op {
    valet_core::model::Op::MoveTo { path } => {
      let dest_path = Path::new(path);
      
      // Ensure destination directory exists
      if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
          .map_err(|e| format!("Failed to create destination directory: {}", e))?;
      }
      
      // Move the file
      fs::rename(source_path, dest_path)
        .map_err(|e| format!("Failed to move file: {}", e))?;
      
      Ok(())
    }
    valet_core::model::Op::CopyTo { path } => {
      let dest_path = Path::new(path);
      
      // Ensure destination directory exists
      if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)
          .map_err(|e| format!("Failed to create destination directory: {}", e))?;
      }
      
      // Copy the file
      fs::copy(source_path, dest_path)
        .map_err(|e| format!("Failed to copy file: {}", e))?;
      
      Ok(())
    }
    _ => {
      Err("Unsupported operation type".to_string())
    }
  }
}

#[tauri::command]
async fn get_config(app: tauri::AppHandle) -> Result<Config, String> {
  let app_cfg_dir = app.path().app_config_dir()
    .map_err(|e| format!("Failed to get config directory: {}", e))?;
  let cfg_path = app_cfg_dir.join("config.json");
  
  if !cfg_path.exists() {
    // Return default config
    return Ok(Config {
      inbox_paths: vec![],
      pause_watchers: false,
      quarantine_retention_days: 30,
    });
  }
  
  let contents = std::fs::read_to_string(&cfg_path)
    .map_err(|e| format!("Failed to read config file: {}", e))?;
  
  serde_json::from_str(&contents)
    .map_err(|e| format!("Failed to parse config JSON: {}", e))
}

#[tauri::command]
async fn save_config(config: Config, app: tauri::AppHandle) -> Result<Config, String> {
  log::info!("Saving config with {} inbox paths", config.inbox_paths.len());
  
  // Validate paths exist
  for path in &config.inbox_paths {
    let path_buf = std::path::PathBuf::from(path);
    if !path_buf.exists() {
      return Err(format!("Path does not exist: {}", path));
    }
  }
  
  let app_cfg_dir = app.path().app_config_dir()
    .map_err(|e| format!("Failed to get config directory: {}", e))?;
  
  // Ensure config directory exists
  std::fs::create_dir_all(&app_cfg_dir)
    .map_err(|e| format!("Failed to create config directory: {}", e))?;
  
  let cfg_path = app_cfg_dir.join("config.json");
  let json = serde_json::to_string_pretty(&config)
    .map_err(|e| format!("Failed to serialize config: {}", e))?;
  
  std::fs::write(&cfg_path, json)
    .map_err(|e| format!("Failed to write config file: {}", e))?;
  
  log::info!("Config saved successfully to {:?}", cfg_path);
  Ok(config)
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StatsResponse {
  stats: valet_core::model::OperationStats,
  recent_operations: Vec<valet_core::model::RecentOperation>,
}

#[tauri::command]
async fn get_statistics(time_range: String, app: tauri::AppHandle) -> Result<StatsResponse, String> {
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
async fn clear_operation_history(app: tauri::AppHandle) -> Result<(), String> {
  log::info!("Clearing operation history");
  
  let state = app.state::<AppState>();
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;
  
  db.clear_operation_history().await
    .map_err(|e| format!("Failed to clear operation history: {}", e))?;
  
  log::info!("Operation history cleared successfully");
  Ok(())
}

#[derive(Clone)]
struct AppState {
  paused: Arc<AtomicBool>,
  _db_path: PathBuf,
}

fn main() {
  tauri::Builder::default()
    .plugin(
      Builder::default()
        .level(LevelFilter::Debug)
        .build()
    )
    .invoke_handler(tauri::generate_handler![
      get_rules,
      upsert_rule, 
      dry_run,
      execute_operations,
      get_config,
      save_config,
      get_statistics,
      clear_operation_history
    ])
    .menu(Menu::new) // optional
    .setup(|app| {
      // --- DB init & seed sample rules ---
      let cfg_dir = config::config_dir().expect("config dir");
      let db_path = cfg_dir.join("valet.sqlite3");
      tauri::async_runtime::block_on(async {
        let db = Db::connect(&db_path).await.expect("db connect");
        db.load_sample_rules_if_empty().await.expect("seed rules");
      });

      // --- Log config path and contents ---
      let app_cfg_dir = app
        .path()
        .app_config_dir()
        .expect("app_config_dir");
      let cfg_path = app_cfg_dir.join("config.json");
      log::info!("Valet config path: {:?}", cfg_path);

      match std::fs::read_to_string(&cfg_path) {
        Ok(txt) => {
          log::info!("Valet config contents: {}", txt);
        }
        Err(e) => {
          log::warn!("Could not read config.json: {:?}", e);
        }
      }

      // --- Start file watcher ---
      match std::fs::read_to_string(&cfg_path) {
        Ok(txt) => {
          // Parse the config to extract the inbox_paths
          if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&txt) {
            if let Some(inbox_paths) = config_json.get("inbox_paths").and_then(|v| v.as_array()) {
              let watch_paths: Vec<PathBuf> = inbox_paths
                .iter()
                .filter_map(|v| v.as_str())
                .map(PathBuf::from)
                .collect();
              
              log::info!("Starting watcher on config paths: {:?}", watch_paths);
              
              let _watcher_handle = tauri::async_runtime::spawn(async move {
                let result = valet_platform::watch_paths(watch_paths.clone(), move |files| {
                  log::info!("🔍 Watcher event detected for files: {:?}", files);
                  // TODO: Process the files here
                }).await;
                
                if let Err(e) = result {
                  log::error!("Failed to start watcher: {:?}", e);
                } else {
                  log::info!("✅ Watcher started successfully");
                }
              });
            } else {
              log::warn!("No inbox_paths found in config");
            }
          } else {
            log::warn!("Failed to parse config JSON");
          }
        }
        Err(e) => {
          log::warn!("Could not read config for watcher setup: {:?}", e);
        }
      }

      // --- App state ---
      app.manage(AppState {
        paused: Arc::new(AtomicBool::new(false)),
        _db_path: db_path.clone(),
      });

      // --- Tray + menu (v2 API) ---
      let dry_run = MenuItemBuilder::with_id("index-dry-run", "Index Downloads (dry run)").build(app)?;
      let pause = CheckMenuItemBuilder::with_id("pause", "Pause watchers")
        .checked(false)
        .build(app)?;
      let preferences = MenuItemBuilder::with_id("preferences", "Preferences…").build(app)?;
      let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
      let tray_menu = Menu::with_items(app, &[&dry_run, &pause, &preferences, &quit])?;

      // Clone variables for the closure
      let cfg_path_clone = cfg_path.clone();
      let db_path_clone = db_path.clone();

      TrayIconBuilder::new()
        .menu(&tray_menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
          "index-dry-run" => {
            log::info!("Tray: dry-run clicked");
            
            // Run dry-run on configured paths
            match std::fs::read_to_string(&cfg_path_clone) {
              Ok(txt) => {
                if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&txt) {
                  if let Some(inbox_paths) = config_json.get("inbox_paths").and_then(|v| v.as_array()) {
                    let watch_paths: Vec<PathBuf> = inbox_paths
                      .iter()
                      .filter_map(|v| v.as_str())
                      .map(PathBuf::from)
                      .collect();
                    
                    // Run dry-run async
                    let db_path_for_async = db_path_clone.clone();
                    tauri::async_runtime::spawn(async move {
                      match valet_core::storage::Db::connect(&db_path_for_async).await {
                        Ok(db) => {
                          match valet_core::engine::dry_run_for_paths(&watch_paths, &db).await {
                            Ok(plan) => {
                              log::info!("🔍 Dry run results for {} action(s):", plan.actions.len());
                              for action in plan.actions {
                                log::info!("  - [{}] {} -> {:?}", action.rule_name, action.file_path, action.op);
                              }
                            }
                            Err(e) => log::error!("Dry run failed: {:?}", e),
                          }
                        }
                        Err(e) => log::error!("Failed to connect to DB for dry run: {:?}", e),
                      }
                    });
                  } else {
                    log::warn!("No inbox_paths found in config for dry run");
                  }
                } else {
                  log::warn!("Failed to parse config JSON for dry run");
                }
              }
              Err(e) => {
                log::warn!("Could not read config for dry run: {:?}", e);
              }
            }
          }
          "pause" => {
            // Toggle handling (you can wire this to engine later)
            let state = app.state::<AppState>();
            let now = !state.paused.load(Ordering::SeqCst);
            state.paused.store(now, Ordering::SeqCst);
            log::info!("Pause toggled -> {}", now);
          }
          "preferences" => {
            log::info!("Tray: preferences clicked");
            // TODO: Open preferences window
          }
          "quit" => app.exit(0),
          _ => {}
        })
        .build(app)?;

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
