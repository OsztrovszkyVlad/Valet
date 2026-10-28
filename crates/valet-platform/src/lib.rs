//! Windows platform layer for Valet: file attributes, locks, volumes, long
//! paths, Recycle Bin and the file watcher. Off Windows every function
//! compiles and returns the neutral answer documented on it, so the workspace
//! builds and tests on Linux.

mod fs_win;
mod recycle;
mod watcher;

pub use fs_win::{
    can_trash, is_cloud_placeholder, is_hidden_or_system, is_locked, is_reparse_point, same_volume,
    to_long_path,
};
pub use recycle::trash_path;
pub use watcher::{DEBOUNCE, WatchCancel, watch_paths};
