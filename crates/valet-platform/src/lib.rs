//! Windows platform layer for Valet: file attributes, locks, volumes, long
//! paths, Recycle Bin (trash and restore), Explorer, per-user auto-start,
//! the file watcher, the license machine id and MSIX package detection. Off
//! Windows every function compiles and returns the neutral answer documented
//! on it, so the workspace builds and tests on Linux.
//!
//! `SystemPlatform` (feature `valet-core`) implements
//! `valet_core::platform::Platform` with these functions.

mod autostart;
mod fs_win;
mod machine;
mod package;
mod recycle;
mod shell;
#[cfg(feature = "valet-core")]
mod system;
mod watcher;

pub use autostart::{
    AUTOSTART_ARG, AUTOSTART_VALUE_NAME, autostart_entry_state, autostart_state, set_autostart,
    set_autostart_entry,
};
pub use fs_win::{
    can_trash, is_cloud_placeholder, is_hidden_or_system, is_locked, is_reparse_point,
    rename_no_replace, same_volume, to_long_path,
};
pub use machine::{machine_id, machine_id_from_guid};
pub use package::is_packaged;
pub use recycle::{DELETED_TOLERANCE_SECS, restore_from_trash, trash_path};
pub use shell::reveal_in_explorer;
#[cfg(feature = "valet-core")]
pub use system::SystemPlatform;
pub use watcher::{DEBOUNCE, WatchCancel, watch_paths};
