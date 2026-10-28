//! Settings commands: watched-folder config (`config.json`), app settings
//! (`app_settings.json`, settings v2) and rule approvals (`approvals.json`).
//!
//! These files are Valet's own; they are the only files Valet overwrites.
//!
//! Errors are stable codes the UI translates: `config.driveRoot`,
//! `config.excludedFolder` and `config.missing` (each as `code: <path>`),
//! `config.noFolders` (from `dry_run`), `settings.io` and `config.io`
//! (detail in the log).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use tauri::Manager;
use uuid::Uuid;
use valet_core::planner::is_excluded_dir_name;
use valet_core::rules::Rule;

use crate::state::{config_dir, AppState};

pub(crate) const ERR_DRIVE_ROOT: &str = "config.driveRoot";
pub(crate) const ERR_EXCLUDED_FOLDER: &str = "config.excludedFolder";
pub(crate) const ERR_MISSING: &str = "config.missing";
pub(crate) const ERR_NO_FOLDERS: &str = "config.noFolders";
pub(crate) const ERR_CONFIG_IO: &str = "config.io";
pub(crate) const ERR_SETTINGS_IO: &str = "settings.io";

const CONFIG_FILE: &str = "config.json";
const SETTINGS_FILE: &str = "app_settings.json";
const APPROVALS_FILE: &str = "approvals.json";

// ---------------------------------------------------------------- config.json

/// `config.json`. Fields of older versions are ignored when loading and
/// dropped on the next save.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub(crate) struct Config {
    /// Watched folders.
    pub(crate) inbox_paths: Vec<String>,
}

/// `path` is a drive or share root: `C:`, `C:\`, `C:/`, `\\server`,
/// `\\server\share` (with or without a trailing separator), the same with a
/// `\\?\` or `\\?\UNC\` prefix, or `/`. Pure string check (no disk access),
/// so Windows paths can be tested on any OS.
pub(crate) fn is_drive_root(path: &str) -> bool {
    let p = path.trim();
    let (p, unc) = if let Some(rest) = p
        .strip_prefix(r"\\?\UNC\")
        .or_else(|| p.strip_prefix("//?/UNC/"))
    {
        (rest, true)
    } else if let Some(rest) = p.strip_prefix(r"\\?\").or_else(|| p.strip_prefix("//?/")) {
        (rest, false)
    } else if let Some(rest) = p.strip_prefix(r"\\").or_else(|| p.strip_prefix("//")) {
        (rest, true)
    } else {
        (p, false)
    };
    let parts: Vec<&str> = p.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    if unc {
        // server, or server\share
        return parts.len() <= 2;
    }
    if path.trim().starts_with('/') && parts.is_empty() {
        return true;
    }
    let b = p.as_bytes();
    let drive = b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':';
    drive && parts.len() == 1
}

/// A component of `path` (split on `\` and `/`) is a default-excluded folder
/// name (`AppData`, `Program Files`, `node_modules`, ...).
pub(crate) fn in_excluded_folder(path: &str) -> bool {
    path.split(['\\', '/']).any(is_excluded_dir_name)
}

/// Checks each watched folder: no drive or share root, nothing inside a
/// default-excluded folder, and it must exist. The user profile folder itself
/// is allowed. Returns `code: <path>` for the first problem.
pub(crate) fn validate_config(
    config: &Config,
    exists: impl Fn(&Path) -> bool,
) -> Result<(), String> {
    for path in &config.inbox_paths {
        let code = if is_drive_root(path) {
            Some(ERR_DRIVE_ROOT)
        } else if in_excluded_folder(path) {
            Some(ERR_EXCLUDED_FOLDER)
        } else if !exists(Path::new(path)) {
            Some(ERR_MISSING)
        } else {
            None
        };
        if let Some(code) = code {
            return Err(format!("{code}: {path}"));
        }
    }
    Ok(())
}

fn read_config(dir: &Path) -> Result<Config, String> {
    let path = dir.join(CONFIG_FILE);
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| io_err(ERR_CONFIG_IO, e))?;
    serde_json::from_str(&text).map_err(|e| io_err(ERR_CONFIG_IO, e))
}

fn write_json<T: Serialize>(dir: &Path, file: &str, value: &T, code: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| io_err(code, e))?;
    let json = serde_json::to_string_pretty(value).map_err(|e| io_err(code, e))?;
    std::fs::write(dir.join(file), json).map_err(|e| io_err(code, e))
}

fn io_err(code: &str, e: impl std::fmt::Display) -> String {
    log::warn!("{code}: {e}");
    code.to_string()
}

/// Watched folders from `config.json` that exist. `config.noFolders` when
/// there are none.
pub(crate) fn watched_folders(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let paths: Vec<PathBuf> = read_config(dir)?
        .inbox_paths
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();
    if paths.is_empty() {
        Err(ERR_NO_FOLDERS.to_string())
    } else {
        Ok(paths)
    }
}

#[tauri::command]
pub(crate) async fn get_config(app: tauri::AppHandle) -> Result<Config, String> {
    read_config(&config_dir(&app)?)
}

fn save_config_in(dir: &Path, config: Config) -> Result<Config, String> {
    validate_config(&config, Path::exists)?;
    write_json(dir, CONFIG_FILE, &config, ERR_CONFIG_IO)?;
    Ok(config)
}

#[tauri::command]
pub(crate) async fn save_config(config: Config, app: tauri::AppHandle) -> Result<Config, String> {
    let dir = config_dir(&app)?;
    let saved = save_config_in(&dir, config)?;
    log::info!(
        "Config saved: {} watched folder(s)",
        saved.inbox_paths.len()
    );
    // Watch the new folders and let auto-apply scan them.
    crate::auto::restart_watcher(
        &app.state::<AppState>(),
        watched_folders(&dir).unwrap_or_default(),
    );
    Ok(saved)
}

// ---------------------------------------------------------------- app_settings.json

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Language {
    #[default]
    Ro,
    En,
}

/// Unknown or malformed values load as the default (Romanian).
fn lenient_language<'de, D: Deserializer<'de>>(d: D) -> Result<Language, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or_default())
}

/// `app_settings.json`, settings v2. Missing fields take their defaults, so
/// files from older versions load; their other fields are dropped on save.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct AppSettings {
    #[serde(deserialize_with = "lenient_language")]
    pub(crate) language: Language,
    pub(crate) notifications_enabled: bool,
    pub(crate) auto_start: bool,
    /// Auto-apply from the watcher; takes effect only with `first_preview_approved`.
    pub(crate) auto_apply_enabled: bool,
    /// Set by the backend once the user approved a preview. `save_app_settings`
    /// keeps the stored value whatever the UI sends.
    pub(crate) first_preview_approved: bool,
    pub(crate) onboarding_done: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: Language::Ro,
            notifications_enabled: true,
            auto_start: false,
            auto_apply_enabled: false,
            first_preview_approved: false,
            onboarding_done: false,
        }
    }
}

/// Stored settings; defaults when the file is missing or unreadable.
pub(crate) fn load_app_settings(dir: &Path) -> AppSettings {
    let path = dir.join(SETTINGS_FILE);
    if !path.exists() {
        return AppSettings::default();
    }
    match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(s) => s,
        Err(e) => {
            log::warn!("app_settings.json unreadable, using defaults: {e}");
            AppSettings::default()
        }
    }
}

fn save_app_settings_in(dir: &Path, incoming: AppSettings) -> Result<AppSettings, String> {
    let stored = load_app_settings(dir);
    let settings = AppSettings {
        first_preview_approved: stored.first_preview_approved,
        ..incoming
    };
    write_json(dir, SETTINGS_FILE, &settings, ERR_SETTINGS_IO)?;
    Ok(settings)
}

#[tauri::command]
pub(crate) async fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    Ok(load_app_settings(&config_dir(&app)?))
}

/// Saves the settings the UI sends, except `first_preview_approved`, which
/// keeps its stored value. Returns the saved settings.
#[tauri::command]
pub(crate) async fn save_app_settings(
    settings: AppSettings,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let saved = save_app_settings_in(&config_dir(&app)?, settings)?;
    log::info!("App settings saved");
    // The tray menu follows the language setting.
    if let Err(e) = crate::setup::refresh_tray(&app) {
        log::warn!("Failed to update the tray menu: {e}");
    }
    Ok(saved)
}

// ---------------------------------------------------------------- approvals.json

/// A rule id with the rule version it had.
pub(crate) type RuleVersion = (Uuid, i32);

/// `approvals.json`: rule id -> the rule version the user approved by
/// executing a preview that contained the rule. Auto-apply runs a rule only
/// at its approved version; editing a rule bumps its version, so it needs a
/// new preview.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(transparent)]
pub(crate) struct Approvals(pub(crate) BTreeMap<Uuid, i32>);

impl Approvals {
    /// `rule` is approved at its current version.
    pub(crate) fn allows(&self, rule: &Rule) -> bool {
        self.0.get(&rule.id) == Some(&rule.version)
    }
}

/// Stored approvals; none when the file is missing or unreadable (so
/// nothing is auto-applied).
pub(crate) fn load_approvals(dir: &Path) -> Approvals {
    let path = dir.join(APPROVALS_FILE);
    if !path.exists() {
        return Approvals::default();
    }
    match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(a) => a,
        Err(e) => {
            log::warn!("approvals.json unreadable, no rule is approved: {e}");
            Approvals::default()
        }
    }
}

/// Records that the user executed a preview containing `rules`: each is
/// approved at the given version (replacing an older approval), and
/// `first_preview_approved` is set in `app_settings.json`.
pub(crate) fn record_preview_approval(dir: &Path, rules: &[RuleVersion]) -> Result<(), String> {
    let mut approvals = load_approvals(dir);
    for (id, version) in rules {
        approvals.0.insert(*id, *version);
    }
    write_json(dir, APPROVALS_FILE, &approvals, ERR_SETTINGS_IO)?;
    let settings = load_app_settings(dir);
    if !settings.first_preview_approved {
        write_json(
            dir,
            SETTINGS_FILE,
            &AppSettings {
                first_preview_approved: true,
                ..settings
            },
            ERR_SETTINGS_IO,
        )?;
    }
    Ok(())
}

/// Removes the approval of `rule_id` (the rule was deleted). Leaves the
/// file alone when it holds no approval for that rule.
pub(crate) fn remove_approval(dir: &Path, rule_id: Uuid) -> Result<(), String> {
    let mut approvals = load_approvals(dir);
    if approvals.0.remove(&rule_id).is_some() {
        write_json(dir, APPROVALS_FILE, &approvals, ERR_SETTINGS_IO)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn remove_approval_drops_only_that_rule() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        remove_approval(dir, Uuid::from_u128(1)).unwrap();
        assert!(!dir.join(APPROVALS_FILE).exists(), "nothing to write");
        record_preview_approval(dir, &[(Uuid::from_u128(1), 2), (Uuid::from_u128(2), 1)]).unwrap();
        remove_approval(dir, Uuid::from_u128(1)).unwrap();
        let a = load_approvals(dir);
        assert!(!a.allows(&rule(1, 2)));
        assert!(a.allows(&rule(2, 1)));
        remove_approval(dir, Uuid::from_u128(9)).unwrap();
        assert_eq!(load_approvals(dir), a);
    }

    #[test]
    fn drive_and_share_roots_are_detected() {
        for root in [
            "C:",
            "C:\\",
            "c:/",
            "D:\\\\",
            "  E:\\ ",
            "\\\\server",
            "\\\\server\\share",
            "\\\\server\\share\\",
            "//server/share",
            "\\\\?\\C:\\",
            "\\\\?\\UNC\\server\\share",
            "/",
        ] {
            assert!(is_drive_root(root), "{root:?}");
        }
        for not_root in [
            "C:\\Users",
            "C:\\Users\\Ana",
            "C:/Users/Ana/Downloads",
            "\\\\server\\share\\in",
            "\\\\?\\C:\\Data",
            "\\\\?\\UNC\\server\\share\\x",
            "/home/ana",
            "Downloads",
            "",
            "CC:\\",
        ] {
            assert!(!is_drive_root(not_root), "{not_root:?}");
        }
    }

    #[test]
    fn excluded_folders_match_by_component() {
        for p in [
            "C:\\Users\\Ana\\AppData\\Roaming\\x",
            "C:\\Program Files\\App",
            "C:\\WINDOWS",
            "D:\\code\\proj\\node_modules",
            "/home/ana/.git/objects",
        ] {
            assert!(in_excluded_folder(p), "{p}");
        }
        for p in [
            "C:\\Users\\Ana",
            "C:\\Users\\Ana\\Downloads",
            "D:\\AppData2",
            "C:\\Users\\Ana\\targets",
        ] {
            assert!(!in_excluded_folder(p), "{p}");
        }
    }

    fn cfg(paths: &[&str]) -> Config {
        Config {
            inbox_paths: paths.iter().map(|p| p.to_string()).collect(),
        }
    }

    #[test]
    fn validate_config_returns_code_and_path() {
        let all = |_: &Path| true;
        assert_eq!(validate_config(&cfg(&["C:\\Users\\Ana"]), all), Ok(()));
        assert_eq!(
            validate_config(&cfg(&["C:\\Users\\Ana", "D:\\"]), all),
            Err("config.driveRoot: D:\\".to_string())
        );
        assert_eq!(
            validate_config(&cfg(&["\\\\nas\\share"]), all),
            Err("config.driveRoot: \\\\nas\\share".to_string())
        );
        assert_eq!(
            validate_config(&cfg(&["C:\\Users\\Ana\\AppData\\Local"]), all),
            Err("config.excludedFolder: C:\\Users\\Ana\\AppData\\Local".to_string())
        );
        assert_eq!(
            validate_config(&cfg(&["C:\\Nope"]), |_| false),
            Err("config.missing: C:\\Nope".to_string())
        );
        // A drive root is reported as such even when it exists.
        assert!(validate_config(&cfg(&["C:\\"]), all)
            .unwrap_err()
            .starts_with(ERR_DRIVE_ROOT));
    }

    #[test]
    fn save_config_writes_only_valid_config_and_returns_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("cfg");
        let watched = tmp.path().join("Downloads");
        std::fs::create_dir_all(&watched).unwrap();
        let ok = cfg(&[watched.to_str().unwrap()]);
        assert_eq!(save_config_in(&dir, ok.clone()).unwrap(), ok);
        assert_eq!(read_config(&dir).unwrap(), ok);

        let missing = tmp.path().join("nope");
        let err = save_config_in(&dir, cfg(&[missing.to_str().unwrap()])).unwrap_err();
        assert!(err.starts_with("config.missing: "), "{err}");
        assert_eq!(read_config(&dir).unwrap(), ok, "unchanged after a refusal");
    }

    #[test]
    fn old_config_files_load_and_drop_unused_fields() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(CONFIG_FILE),
            r#"{"inbox_paths":["C:\\in"],"pause_watchers":true,"quarantine_retention_days":30}"#,
        )
        .unwrap();
        let c = read_config(tmp.path()).unwrap();
        assert_eq!(c, cfg(&["C:\\in"]));
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            json!({ "inbox_paths": ["C:\\in"] })
        );
        assert_eq!(
            read_config(&tmp.path().join("none")).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn watched_folders_without_existing_folders_is_no_folders() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(watched_folders(tmp.path()).unwrap_err(), ERR_NO_FOLDERS);
        let dir = tmp.path().join("in");
        std::fs::create_dir_all(&dir).unwrap();
        let gone = tmp.path().join("gone");
        write_json(
            tmp.path(),
            CONFIG_FILE,
            &cfg(&[gone.to_str().unwrap()]),
            ERR_CONFIG_IO,
        )
        .unwrap();
        assert_eq!(watched_folders(tmp.path()).unwrap_err(), ERR_NO_FOLDERS);
        write_json(
            tmp.path(),
            CONFIG_FILE,
            &cfg(&[gone.to_str().unwrap(), dir.to_str().unwrap()]),
            ERR_CONFIG_IO,
        )
        .unwrap();
        assert_eq!(watched_folders(tmp.path()).unwrap(), vec![dir]);
    }

    #[test]
    fn settings_defaults() {
        let s = AppSettings::default();
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            json!({
                "language": "ro",
                "notifications_enabled": true,
                "auto_start": false,
                "auto_apply_enabled": false,
                "first_preview_approved": false,
                "onboarding_done": false,
            })
        );
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(load_app_settings(tmp.path()), s, "missing file");
    }

    #[test]
    fn old_settings_files_keep_known_fields() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(SETTINGS_FILE),
            r#"{"theme":"dark","compact_mode":true,"notifications_enabled":false,
               "notification_types":{"errors":true},"auto_start":true,"log_level":"info"}"#,
        )
        .unwrap();
        let s = load_app_settings(tmp.path());
        assert_eq!(
            s,
            AppSettings {
                notifications_enabled: false,
                auto_start: true,
                ..AppSettings::default()
            }
        );
    }

    #[test]
    fn unknown_language_and_corrupt_file_load_as_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(SETTINGS_FILE),
            r#"{"language":"de","onboarding_done":true}"#,
        )
        .unwrap();
        let s = load_app_settings(tmp.path());
        assert_eq!(s.language, Language::Ro);
        assert!(s.onboarding_done);
        let en: AppSettings = serde_json::from_value(json!({ "language": "en" })).unwrap();
        assert_eq!(en.language, Language::En);

        std::fs::write(tmp.path().join(SETTINGS_FILE), "{ not json").unwrap();
        assert_eq!(load_app_settings(tmp.path()), AppSettings::default());
    }

    fn rule(n: u128, version: i32) -> Rule {
        serde_json::from_value(json!({
            "id": Uuid::from_u128(n),
            "name": format!("r{n}"),
            "enabled": true,
            "conditions": [],
            "actions": [{ "type": "report" }],
            "version": version,
        }))
        .unwrap()
    }

    #[test]
    fn approval_is_per_version_and_sets_first_preview_approved() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert_eq!(load_approvals(dir), Approvals::default());
        assert!(!load_app_settings(dir).first_preview_approved);

        record_preview_approval(dir, &[(Uuid::from_u128(1), 2)]).unwrap();
        let a = load_approvals(dir);
        assert!(a.allows(&rule(1, 2)));
        assert!(!a.allows(&rule(1, 3)), "an edited rule needs a new preview");
        assert!(!a.allows(&rule(1, 1)));
        assert!(!a.allows(&rule(2, 2)), "never previewed");
        assert!(load_app_settings(dir).first_preview_approved);

        // A later preview approves the new version and keeps other rules.
        record_preview_approval(dir, &[(Uuid::from_u128(1), 3), (Uuid::from_u128(2), 1)]).unwrap();
        let a = load_approvals(dir);
        assert!(a.allows(&rule(1, 3)) && !a.allows(&rule(1, 2)));
        assert!(a.allows(&rule(2, 1)));

        // An unreadable file approves nothing.
        std::fs::write(dir.join(APPROVALS_FILE), "{ nope").unwrap();
        assert_eq!(load_approvals(dir), Approvals::default());
    }

    #[test]
    fn save_keeps_stored_first_preview_approved() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let ui = AppSettings {
            language: Language::En,
            auto_apply_enabled: true,
            first_preview_approved: true,
            onboarding_done: true,
            ..AppSettings::default()
        };
        let saved = save_app_settings_in(dir, ui.clone()).unwrap();
        assert!(!saved.first_preview_approved, "the UI cannot set it");
        assert_eq!(saved.language, Language::En);
        assert!(saved.auto_apply_enabled && saved.onboarding_done);
        assert_eq!(load_app_settings(dir), saved);

        // Once the backend stored true, the UI cannot clear it either.
        write_json(
            dir,
            SETTINGS_FILE,
            &AppSettings {
                first_preview_approved: true,
                ..saved
            },
            ERR_SETTINGS_IO,
        )
        .unwrap();
        let saved = save_app_settings_in(
            dir,
            AppSettings {
                first_preview_approved: false,
                ..ui
            },
        )
        .unwrap();
        assert!(saved.first_preview_approved);
    }
}
