//! App startup: DB init and sample rules, config logging, file watcher, app state, tray,
//! main window visibility.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use tauri::{
    menu::{Menu, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};
use valet_core::{executor, storage::Db};

use crate::commands::platform::launched_minimized;
use crate::i18n;
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

    // --- Tray ---
    let tray_menu = tray_menu(app.handle())?;
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&tray_menu)
        .tooltip(i18n::text(i18n::current_lang(app)).tray_tooltip)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_SHOW => show_main_window(app),
            MENU_PAUSE => {
                let state = app.state::<AppState>();
                let paused = !state.paused.load(Ordering::SeqCst);
                state.paused.store(paused, Ordering::SeqCst);
                log::info!("Automatic organizing paused: {}", paused);
                if let Err(e) = refresh_tray(app) {
                    log::warn!("Failed to update the tray menu: {}", e);
                }
            }
            MENU_QUIT => {
                app.state::<AppState>().stop_watcher();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;

    // --- Main window: hidden when started by auto-start, shown otherwise ---
    if let Some(window) = app.get_webview_window("main") {
        if launched_minimized(std::env::args()) {
            log::info!("Started minimized to the tray");
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    Ok(())
}

const TRAY_ID: &str = "main";
const MENU_SHOW: &str = "show";
const MENU_PAUSE: &str = "pause";
const MENU_QUIT: &str = "quit";

/// The tray menu in the current language, with the pause item matching
/// `AppState.paused`.
fn tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let text = i18n::text(i18n::current_lang(app));
    let paused = app
        .try_state::<AppState>()
        .is_some_and(|s| s.paused.load(Ordering::SeqCst));
    let show = MenuItemBuilder::with_id(MENU_SHOW, text.tray_show).build(app)?;
    let pause_label = if paused {
        text.tray_resume
    } else {
        text.tray_pause
    };
    let pause = MenuItemBuilder::with_id(MENU_PAUSE, pause_label).build(app)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id(MENU_QUIT, text.tray_quit).build(app)?;
    Menu::with_items(app, &[&show, &pause, &separator, &quit])
}

/// Rebuilds the tray menu and tooltip from the current language and pause
/// state. Call after the language in `app_settings.json` changes.
pub(crate) fn refresh_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    tray.set_menu(Some(tray_menu(app)?))?;
    tray.set_tooltip(Some(i18n::text(i18n::current_lang(app)).tray_tooltip))?;
    Ok(())
}

/// Shows, restores and focuses the main window.
pub(crate) fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
