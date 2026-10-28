//! Shared application state managed by Tauri.

use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};

#[derive(Clone)]
pub(crate) struct AppState {
  pub(crate) paused: Arc<AtomicBool>,
  pub(crate) _db_path: PathBuf,
}
