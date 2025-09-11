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

use valet_core::{config, storage::Db};

#[derive(Clone)]
struct AppState {
  paused: Arc<AtomicBool>,
  _db_path: PathBuf,
}

fn main() {
  tauri::Builder::default()
    .menu(|app| Menu::new(app)) // optional
    .setup(|app| {
      // --- DB init & seed sample rules ---
      let cfg_dir = config::config_dir().expect("config dir");
      let db_path = cfg_dir.join("valet.sqlite3");
      tauri::async_runtime::block_on(async {
        let db = Db::connect(&db_path).await.expect("db connect");
        db.load_sample_rules_if_empty().await.expect("seed rules");
      });

      // --- App state ---
      app.manage(AppState {
        paused: Arc::new(AtomicBool::new(false)),
        _db_path: db_path,
      });

      // --- Tray + menu (v2 API) ---
      let pause = CheckMenuItemBuilder::with_id("pause", "Pause watchers")
        .checked(false)
        .build(app)?;
      let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
      let tray_menu = Menu::with_items(app, &[&pause, &quit])?;

      TrayIconBuilder::new()
        .menu(&tray_menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
          "pause" => {
            // Toggle handling (you can wire this to engine later)
            let state = app.state::<AppState>();
            let now = !state.paused.load(Ordering::SeqCst);
            state.paused.store(now, Ordering::SeqCst);
            println!("Pause toggled -> {}", now);
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
