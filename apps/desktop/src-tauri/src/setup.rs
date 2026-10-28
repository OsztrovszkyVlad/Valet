//! App startup: DB init and sample rules, config logging, file watcher, app state, tray menu.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use tauri::{
    menu::{CheckMenuItemBuilder, Menu, MenuItemBuilder},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
use valet_core::{executor, storage::Db};

use crate::commands::operations::plan_for_paths;
use crate::commands::platform::{send_notification, show_in_explorer_internal};
use crate::state::{config_dir, AppState};

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // --- DB init & seed sample rules ---
    let cfg_dir = config_dir(app).expect("config dir");
    let db_path = cfg_dir.join("valet.sqlite3");
    let state = AppState::new(db_path.clone());
    tauri::async_runtime::block_on(async {
        let db = Db::connect(&db_path).await.expect("db connect");
        db.load_sample_rules_if_empty().await.expect("seed rules");
        // Finish or roll back operations a crash interrupted, before the
        // watcher can start new ones.
        match executor::recover(&db.journal(), state.platform.as_ref()).await {
            Ok(report) if report.batch_id.is_some() => log::info!("{}", report.log_line()),
            Ok(_) => {}
            Err(e) => log::error!("Recovery of interrupted operations failed: {:#}", e),
        }
    });

    // INFO logs carry counts, never paths or config contents.
    let cfg_path = cfg_dir.join("config.json");

    // --- App state (before the watcher, which stores its handle here) ---
    let watcher_slot = state.watcher.clone();
    app.manage(state);

    // --- Start file watcher ---
    match std::fs::read_to_string(&cfg_path) {
        Ok(txt) => {
            // Parse the config to extract the inbox_paths
            if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&txt) {
                if let Some(inbox_paths) = config_json.get("inbox_paths").and_then(|v| v.as_array())
                {
                    let watch_paths: Vec<PathBuf> = inbox_paths
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(PathBuf::from)
                        .collect();

                    log::info!(
                        "config loaded: {} inbox folder(s), starting watcher",
                        watch_paths.len()
                    );
                    log::debug!("watch paths: {:?}", watch_paths);

                    let _watcher_handle = tauri::async_runtime::spawn(async move {
                        let result =
                            valet_platform::watch_paths(watch_paths.clone(), move |files| {
                                log::info!("watcher event: {} file(s)", files.len());
                                log::debug!("watcher event files: {:?}", files);
                                // TODO: Process the files here
                            })
                            .await;

                        match result {
                            Ok(handle) => {
                                *watcher_slot
                                    .lock()
                                    .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                                    Some(handle);
                                log::info!("✅ Watcher started successfully");
                            }
                            Err(e) => {
                                // The error chain names the folder that failed.
                                log::error!("Failed to start watcher");
                                log::debug!("watcher error: {:#}", e);
                            }
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

    // --- Tray + menu (v2 API) ---
    let dry_run =
        MenuItemBuilder::with_id("index-dry-run", "Index Downloads (dry run)").build(app)?;
    let show_stats = MenuItemBuilder::with_id("show-stats", "Show Statistics").build(app)?;
    let open_config_folder =
        MenuItemBuilder::with_id("open-config", "Open Config Folder").build(app)?;
    let separator1 = MenuItemBuilder::with_id("sep1", "").build(app)?;
    let pause = CheckMenuItemBuilder::with_id("pause", "Pause watchers")
        .checked(false)
        .build(app)?;
    let separator2 = MenuItemBuilder::with_id("sep2", "").build(app)?;
    let preferences = MenuItemBuilder::with_id("preferences", "Preferences…").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let tray_menu = Menu::with_items(
        app,
        &[
            &dry_run,
            &show_stats,
            &open_config_folder,
            &separator1,
            &pause,
            &separator2,
            &preferences,
            &quit,
        ],
    )?;

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
                            if let Some(inbox_paths) =
                                config_json.get("inbox_paths").and_then(|v| v.as_array())
                            {
                                let watch_paths: Vec<PathBuf> = inbox_paths
                                    .iter()
                                    .filter_map(|v| v.as_str())
                                    .map(PathBuf::from)
                                    .collect();

                                // Run dry-run async
                                let db_path_for_async = db_path_clone.clone();
                                let platform = app.state::<AppState>().platform.clone();
                                tauri::async_runtime::spawn(async move {
                                    match valet_core::storage::Db::connect(&db_path_for_async).await
                                    {
                                        Ok(db) => {
                                            match plan_for_paths(&watch_paths, &db, platform.as_ref()).await {
                                                Ok(plan) => {
                                                    log::info!(
                                                        "🔍 Dry run results for {} operation(s), {} skipped:",
                                                        plan.ops.len(),
                                                        plan.skipped.len()
                                                    );
                                                    for op in plan.ops {
                                                        log::debug!(
                                                            "  - [{}] {} -> {} ({:?})",
                                                            op.rule_name,
                                                            op.source.display(),
                                                            op.dest.display(),
                                                            op.kind
                                                        );
                                                    }
                                                }
                                                Err(e) => log::error!("Dry run failed: {:?}", e),
                                            }
                                        }
                                        Err(e) => log::error!(
                                            "Failed to connect to DB for dry run: {:?}",
                                            e
                                        ),
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
            "show-stats" => {
                log::info!("Tray: show-stats clicked");
                // Bring window to front and navigate to stats
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("navigate-to-stats", ());
                }
            }
            "open-config" => {
                log::info!("Tray: open-config clicked");
                if let Ok(dir) = config_dir(app) {
                    let _ = show_in_explorer_internal(&dir.to_string_lossy());
                }
            }
            "pause" => {
                // Toggle handling (you can wire this to engine later)
                let state = app.state::<AppState>();
                let now = !state.paused.load(Ordering::SeqCst);
                state.paused.store(now, Ordering::SeqCst);
                log::info!("Pause toggled -> {}", now);

                // Send notification about pause state
                let status = if now { "paused" } else { "resumed" };
                // Pre-existing: the future is never awaited, so no notification is sent. Listed in docs/reports/phase-0.md.
                #[allow(clippy::let_underscore_future)]
                let _ = send_notification(
                    "Valet File Manager".to_string(),
                    format!("File monitoring {}", status),
                    app.clone(),
                );
            }
            "preferences" => {
                log::info!("Tray: preferences clicked");
                // Bring window to front and navigate to preferences
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("navigate-to-preferences", ());
                }
            }
            "quit" => {
                app.state::<AppState>().stop_watcher();
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
