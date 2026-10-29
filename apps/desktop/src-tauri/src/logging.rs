//! Log setup (PLAN 4.5) and the panic hook.
//!
//! Logs go to the app log directory (`TargetKind::LogDir`): on Windows
//! `%LOCALAPPDATA%\ro.valetapp.desktop\logs\valet.log`, rotated at 2 MB,
//! 5 files at most (the active one and 4 rotated ones). Debug builds also log
//! to stdout.
//!
//! Level: INFO in release builds, DEBUG when `debug_logging` is on in the
//! settings (DEBUG lines may contain paths; INFO lines must not). The level
//! changes live: [`set_debug_logging`] moves both the filter of the file
//! logger and `log::max_level`. Debug builds always log at DEBUG.

use std::panic::PanicHookInfo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use log::LevelFilter;
use regex::Regex;
use tauri::{plugin::TauriPlugin, Runtime};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// What the log plugin is built with. Kept as plain values so a test can
/// check them; [`plugin`] turns them into the plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogSettings {
    /// Log file name without `.log`, in the app log directory.
    pub(crate) file_name: &'static str,
    /// A log file is rotated before it grows past this many bytes.
    pub(crate) max_file_size: u128,
    /// Rotated files kept besides the active one. tauri-plugin-log 2.10.1
    /// `KeepSome(n)` keeps n rotated files plus the active file, so 4 gives
    /// 5 files of at most 2 MB.
    pub(crate) keep_rotated: usize,
    /// Also log to stdout (debug builds).
    pub(crate) stdout: bool,
    /// Crates capped at INFO even with DEBUG on: sqlx logs every SQL
    /// statement and the HTTP stack every connection, which would push the
    /// useful lines out of the 10 MB of logs.
    pub(crate) info_only: &'static [&'static str],
}

pub(crate) fn log_settings() -> LogSettings {
    LogSettings {
        file_name: "valet",
        max_file_size: 2_000_000,
        keep_rotated: 4,
        stdout: cfg!(debug_assertions),
        info_only: &["sqlx", "hyper", "hyper_util", "reqwest", "rustls"],
    }
}

/// DEBUG records pass the file logger only while this is set.
static DEBUG_ON: AtomicBool = AtomicBool::new(cfg!(debug_assertions));

/// The level for the `debug_logging` setting. Debug builds always use DEBUG.
pub(crate) fn level_for(debug_logging: bool) -> LevelFilter {
    if debug_logging || cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    }
}

/// Applies the `debug_logging` setting now, without a restart.
pub(crate) fn set_debug_logging(enabled: bool) {
    let level = level_for(enabled);
    DEBUG_ON.store(level >= LevelFilter::Debug, Ordering::SeqCst);
    log::set_max_level(level);
}

/// The log plugin. Its logger accepts DEBUG; the filter drops DEBUG records
/// unless [`set_debug_logging`] turned them on, so the level can change at
/// run time (the plugin has no API for that).
pub(crate) fn plugin<R: Runtime>(settings: &LogSettings) -> TauriPlugin<R> {
    let mut targets = vec![Target::new(TargetKind::LogDir {
        file_name: Some(settings.file_name.to_string()),
    })];
    if settings.stdout {
        targets.push(Target::new(TargetKind::Stdout));
    }
    let mut builder = tauri_plugin_log::Builder::new();
    for &target in settings.info_only {
        builder = builder.level_for(target, LevelFilter::Info);
    }
    builder
        .targets(targets)
        .max_file_size(settings.max_file_size)
        .rotation_strategy(RotationStrategy::KeepSome(settings.keep_rotated))
        .level(LevelFilter::Debug)
        .filter(|metadata| metadata.level() <= log::Level::Info || DEBUG_ON.load(Ordering::SeqCst))
        .build()
}

// ---------------------------------------------------------------- panic hook

/// Logs every panic at ERROR (message with paths replaced, source file and
/// line), flushes the log, then runs the previous hook (the default one
/// prints to stderr).
///
/// The hook does not touch the database: a batch the panic interrupted keeps
/// its `in_progress` journal rows, and journal recovery at the next start
/// finishes or rolls back each of them (`executor::recover` in `setup.rs`),
/// so every file ends up exactly once, at its source or its destination.
pub(crate) fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("{}", panic_line(info));
        log::logger().flush();
        previous(info);
    }));
}

/// `Panic at <file>:<line>:<column>: <message>`.
pub(crate) fn panic_line(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("(no message)");
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", short_source(l.file()), l.line(), l.column()))
        .unwrap_or_else(|| "unknown location".to_string());
    format!("Panic at {location}: {}", scrub_paths(message))
}

/// The last three components of a source path. Sources of dependencies are
/// absolute paths on the build machine (cargo registry); the tail is enough
/// to find the line.
fn short_source(file: &str) -> String {
    let parts: Vec<&str> = file.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    if parts.len() <= 3 {
        return parts.join("/");
    }
    parts[parts.len() - 3..].join("/")
}

/// Replaces absolute paths (`C:\...`, `\\server\...`, `\\?\...`, `/a/b`)
/// in a panic message with `<path>`: a message may carry an error that names
/// a user file.
pub(crate) fn scrub_paths(message: &str) -> String {
    static PATH: OnceLock<Regex> = OnceLock::new();
    let re = PATH.get_or_init(|| {
        Regex::new(
            r#"(?x)
            (?P<pre>^|[\s"'(=:])
            (?:
                (?:\\\\[?.]\\)?[A-Za-z]:[\\/][^"'<>|\r\n]*   # C:\dir\file, \\?\C:\...
              | \\\\[^\s"'<>|\\]+\\[^"'<>|\r\n]*           # \\server\share\...
              | /[^\s"'/]+/[^\s"']*                       # /home/user/...
            )
            "#,
        )
        .expect("path regex")
    });
    re.replace_all(message, "${pre}<path>").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn log_settings_values() {
        let s = log_settings();
        assert_eq!(s.file_name, "valet");
        assert_eq!(s.max_file_size, 2_000_000);
        // 4 rotated + the active file = 5 files of 2 MB.
        assert_eq!(s.keep_rotated + 1, 5);
        assert_eq!(s.stdout, cfg!(debug_assertions));
        assert!(s.info_only.contains(&"sqlx"));
    }

    #[test]
    fn level_follows_debug_logging() {
        assert_eq!(level_for(true), LevelFilter::Debug);
        let want = if cfg!(debug_assertions) {
            LevelFilter::Debug
        } else {
            LevelFilter::Info
        };
        assert_eq!(level_for(false), want);
    }

    #[test]
    fn scrub_replaces_absolute_paths() {
        assert_eq!(
            scrub_paths(r"db connect: unable to open C:\Users\Ana\AppData\valet.sqlite3"),
            "db connect: unable to open <path>"
        );
        assert_eq!(
            scrub_paths(r#"failed "\\?\C:\Users\Ana\x.txt" here"#),
            r#"failed "<path>" here"#
        );
        assert_eq!(scrub_paths(r"share \\nas\docs\a.pdf"), "share <path>");
        assert_eq!(
            scrub_paths("open /home/ana/Downloads/a.pdf failed"),
            "open <path> failed"
        );
        assert_eq!(
            scrub_paths("index out of bounds: 3/4"),
            "index out of bounds: 3/4"
        );
        assert_eq!(scrub_paths("plain message"), "plain message");
    }

    #[test]
    fn short_source_keeps_the_tail() {
        assert_eq!(short_source("src/setup.rs"), "src/setup.rs");
        assert_eq!(
            short_source(r"C:\Users\runner\.cargo\registry\src\idx\tokio-1.0\src\lib.rs"),
            "tokio-1.0/src/lib.rs"
        );
    }

    /// Records ERROR lines; the only logger set in this test binary.
    struct Capture(Mutex<Vec<String>>);

    impl log::Log for Capture {
        fn enabled(&self, m: &log::Metadata<'_>) -> bool {
            m.level() <= log::Level::Error
        }
        fn log(&self, r: &log::Record<'_>) {
            if self.enabled(r.metadata()) {
                self.0.lock().unwrap().push(r.args().to_string());
            }
        }
        fn flush(&self) {}
    }

    static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

    #[test]
    fn panic_hook_logs_message_and_location() {
        log::set_logger(&CAPTURE).expect("no other logger in tests");
        log::set_max_level(LevelFilter::Error);
        install_panic_hook();

        let result = std::panic::catch_unwind(|| {
            panic!(r"valet-test-panic reading C:\Users\Ana\secret.txt");
        });
        assert!(result.is_err());

        let lines = CAPTURE.0.lock().unwrap().clone();
        let line = lines
            .iter()
            .find(|l| l.contains("valet-test-panic"))
            .expect("panic logged");
        assert!(line.starts_with("Panic at "), "{line}");
        assert!(line.contains("src/logging.rs:"), "{line}");
        assert!(line.ends_with("valet-test-panic reading <path>"), "{line}");
        assert!(!line.contains("Ana"), "{line}");
    }
}
