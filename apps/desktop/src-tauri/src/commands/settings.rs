//! Settings commands: watched-folder config (`config.json`), app settings
//! (`app_settings.json`, settings v2) and rule approvals (`approvals.json`).
//!
//! These files are Valet's own; they are the only files Valet overwrites.
//!
//! `get_config` returns `{ inbox_paths, invalid_paths: [{ path, code }],
//! unavailable_paths }`: entries of a hand-edited `config.json` that
//! `save_config` would refuse stay in the file but are not watched or
//! planned; folders that do not exist right now (an unplugged drive) are
//! saved, reported as unavailable and skipped until they exist again (see
//! [`ConfigView`]).
//!
//! Errors are stable codes the UI translates: `config.driveRoot` and
//! `config.excludedFolder` (each as `code: <path>`), `config.missing` (only
//! from onboarding, for an unknown folder token, see `presets.rs`),
//! `config.noFolders` (from `dry_run`), `settings.invalidChannel` (from
//! `save_app_settings`), `settings.io` and `config.io` (detail in the log).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use tauri::Manager;
use uuid::Uuid;
use valet_core::entitlements::{check_folders, Limits};
use valet_core::license::PlanTier;
use valet_core::planner::is_excluded_dir_name;
use valet_core::rules::Rule;

use crate::commands::CmdError;
use crate::state::{config_dir, AppState};

pub(crate) const ERR_DRIVE_ROOT: &str = "config.driveRoot";
pub(crate) const ERR_EXCLUDED_FOLDER: &str = "config.excludedFolder";
pub(crate) const ERR_MISSING: &str = "config.missing";
pub(crate) const ERR_NO_FOLDERS: &str = "config.noFolders";
pub(crate) const ERR_CONFIG_IO: &str = "config.io";
pub(crate) const ERR_SETTINGS_IO: &str = "settings.io";
pub(crate) const ERR_INVALID_CHANNEL: &str = "settings.invalidChannel";

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

/// The problem with one watched-folder entry, if any: a drive or share root
/// (`config.driveRoot`) or inside a default-excluded folder
/// (`config.excludedFolder`), checked in that order. The user profile folder
/// itself is allowed. Pure string check: a folder that does not exist is not
/// a problem here (see [`split_config`]).
pub(crate) fn path_problem(path: &str) -> Option<&'static str> {
    if is_drive_root(path) {
        Some(ERR_DRIVE_ROOT)
    } else if in_excluded_folder(path) {
        Some(ERR_EXCLUDED_FOLDER)
    } else {
        None
    }
}

/// Checks each watched folder with [`path_problem`]. Returns `code: <path>`
/// for the first problem.
pub(crate) fn validate_config(config: &Config) -> Result<(), String> {
    for path in &config.inbox_paths {
        if let Some(code) = path_problem(path) {
            return Err(format!("{code}: {path}"));
        }
    }
    Ok(())
}

/// A `config.json` entry that is not watched, with the reason code.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct InvalidPath {
    pub(crate) path: String,
    pub(crate) code: &'static str,
}

/// The entries of `config` by what Valet does with them, each in
/// configuration order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ConfigSplit {
    /// Watched and planned.
    pub(crate) available: Vec<PathBuf>,
    /// Refused by [`path_problem`] (hand-edited `config.json`).
    pub(crate) invalid: Vec<InvalidPath>,
    /// Valid but not existing right now; picked up again once they exist.
    pub(crate) unavailable: Vec<String>,
    /// Valid but beyond the plan's folder limit (Free: after the first 2 in
    /// `inbox_paths` order). Not watched or planned; approvals are kept.
    pub(crate) inactive: Vec<String>,
}

/// Splits the entries of `config`: [`path_problem`] first (the checks
/// `save_config` applies, for a hand-edited `config.json`), then the plan's
/// `folder_limit` (entries at position `folder_limit` and later are
/// inactive, as `entitlements::split_active_folders`), then `exists`.
pub(crate) fn split_config(
    config: &Config,
    exists: impl Fn(&Path) -> bool,
    folder_limit: Option<usize>,
) -> ConfigSplit {
    let mut split = ConfigSplit::default();
    for (i, path) in config.inbox_paths.iter().enumerate() {
        match path_problem(path) {
            Some(code) => split.invalid.push(InvalidPath {
                path: path.clone(),
                code,
            }),
            None if folder_limit.is_some_and(|n| i >= n) => split.inactive.push(path.clone()),
            None if exists(Path::new(path)) => split.available.push(PathBuf::from(path)),
            None => split.unavailable.push(path.clone()),
        }
    }
    split
}

/// The folder limit of `plan` (`None` = unlimited).
pub(crate) fn folder_limit(plan: &PlanTier) -> Option<usize> {
    Limits::for_plan(plan).folders
}

pub(crate) fn read_config(dir: &Path) -> Result<Config, String> {
    let path = dir.join(CONFIG_FILE);
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| io_err(ERR_CONFIG_IO, e))?;
    serde_json::from_str(&text).map_err(|e| io_err(ERR_CONFIG_IO, e))
}

/// Writes `config` as it is (no validation). For a change that keeps the
/// same entries (reordering by `apply_plan_choice`).
pub(crate) fn write_config(dir: &Path, config: &Config) -> Result<(), String> {
    write_json(dir, CONFIG_FILE, config, ERR_CONFIG_IO)
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

/// The available watched folders from `config.json` ([`split_config`] with
/// the disk as it is now and the plan's `folder_limit`). Invalid,
/// inactive and unavailable entries are skipped and only counted in the
/// log. `config.noFolders` when none is left. Read again on every call, so a
/// folder that reappears is picked up by the next caller (manual preview,
/// `save_config`, the auto-apply rescan).
pub(crate) fn watched_folders(
    dir: &Path,
    folder_limit: Option<usize>,
) -> Result<Vec<PathBuf>, String> {
    watched_folders_in(dir, Path::exists, folder_limit)
}

fn watched_folders_in(
    dir: &Path,
    exists: impl Fn(&Path) -> bool,
    folder_limit: Option<usize>,
) -> Result<Vec<PathBuf>, String> {
    let split = split_config(&read_config(dir)?, exists, folder_limit);
    if !split.inactive.is_empty() {
        log::info!(
            "{} watched folder(s) inactive on the current plan",
            split.inactive.len()
        );
    }
    if !split.invalid.is_empty() {
        log::info!(
            "{} watched folder entr(ies) in config.json ignored (drive root or excluded folder)",
            split.invalid.len()
        );
    }
    if !split.unavailable.is_empty() {
        log::info!(
            "{} watched folder(s) unavailable, skipped until they exist again",
            split.unavailable.len()
        );
    }
    if split.available.is_empty() {
        Err(ERR_NO_FOLDERS.to_string())
    } else {
        Ok(split.available)
    }
}

/// What `get_config` returns: the stored config (`inbox_paths`, unchanged,
/// every entry included) plus `invalid_paths`, the entries Valet refuses,
/// each `{ path, code }` with code `config.driveRoot` or
/// `config.excludedFolder`, `unavailable_paths`, the valid entries that do
/// not exist right now (not watched or planned until they do), and
/// `inactive_paths`, the valid entries beyond the plan's folder limit (Free:
/// after the first 2; not watched or planned while on Free).
#[derive(Serialize, Debug, Clone, PartialEq)]
pub(crate) struct ConfigView {
    #[serde(flatten)]
    pub(crate) config: Config,
    pub(crate) invalid_paths: Vec<InvalidPath>,
    pub(crate) unavailable_paths: Vec<String>,
    pub(crate) inactive_paths: Vec<String>,
}

fn config_view(
    dir: &Path,
    exists: impl Fn(&Path) -> bool,
    folder_limit: Option<usize>,
) -> Result<ConfigView, String> {
    let config = read_config(dir)?;
    let split = split_config(&config, exists, folder_limit);
    Ok(ConfigView {
        config,
        invalid_paths: split.invalid,
        unavailable_paths: split.unavailable,
        inactive_paths: split.inactive,
    })
}

#[tauri::command]
pub(crate) async fn get_config(app: tauri::AppHandle) -> Result<ConfigView, String> {
    let plan = crate::licensing::current_plan(&app).await?;
    config_view(&config_dir(&app)?, Path::exists, folder_limit(&plan))
}

/// Validates and writes `config`. A folder that does not exist is saved
/// (unavailable, see [`split_config`]). On Free, adding a folder beyond the
/// limit is refused with `plan.limit` (removing and reordering are allowed,
/// see `entitlements::check_folders`). Before writing, approvals lose the
/// folders `config` no longer has (see [`prune_approval_roots`]), so an
/// unavailable or inactive folder keeps its approvals; if that fails nothing
/// is written.
pub(crate) fn save_config_in(
    dir: &Path,
    config: Config,
    plan: &PlanTier,
) -> Result<Config, CmdError> {
    validate_config(&config)?;
    let stored = read_config(dir).unwrap_or_default();
    check_folders(&stored.inbox_paths, &config.inbox_paths, plan)?;
    prune_approval_roots(dir, &config.inbox_paths)?;
    write_json(dir, CONFIG_FILE, &config, ERR_CONFIG_IO)?;
    Ok(config)
}

#[tauri::command]
pub(crate) async fn save_config(config: Config, app: tauri::AppHandle) -> Result<Config, CmdError> {
    let dir = config_dir(&app)?;
    let plan = crate::licensing::current_plan(&app).await?;
    let saved = save_config_in(&dir, config, &plan)?;
    log::info!(
        "Config saved: {} watched folder(s)",
        saved.inbox_paths.len()
    );
    // Watch the new folders and let auto-apply scan them.
    crate::auto::restart_watcher(
        &app.state::<AppState>(),
        watched_folders(&dir, folder_limit(&plan)).unwrap_or_default(),
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

/// Update channels the updater knows.
pub(crate) const UPDATE_CHANNELS: [&str; 2] = ["stable", "beta"];

/// The version of the running app.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A stored channel Valet does not know (hand-edited file) loads as unset,
/// so the default for the running version applies.
fn lenient_channel<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_str()
        .filter(|c| UPDATE_CHANNELS.contains(c))
        .map(str::to_owned))
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
    /// `"stable"` or `"beta"`. Unset until the settings are saved once; then
    /// [`effective_update_channel`] applies. `get_app_settings` and
    /// `save_app_settings` always return it set.
    #[serde(
        deserialize_with = "lenient_channel",
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) update_channel: Option<String>,
    /// Log at DEBUG level (may contain paths). Off by default.
    pub(crate) debug_logging: bool,
    /// Automatic update checks (30 s after start, then every 24 h). Off:
    /// only "Check now" (`check_for_update`) asks. Files without the field
    /// load as on.
    pub(crate) auto_update_check: bool,
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
            update_channel: None,
            debug_logging: false,
            auto_update_check: true,
        }
    }
}

/// The channel the updater uses: the stored one, else `"beta"` for a
/// pre-release app version (one with `-`, e.g. `0.3.0-beta.1`) and
/// `"stable"` otherwise.
pub(crate) fn effective_update_channel(settings: &AppSettings) -> &str {
    channel_for(settings, APP_VERSION)
}

fn channel_for<'a>(settings: &'a AppSettings, version: &str) -> &'a str {
    match settings.update_channel.as_deref() {
        Some(c) => c,
        None if version.contains('-') => "beta",
        None => "stable",
    }
}

/// `settings` with `update_channel` set to [`effective_update_channel`].
fn with_channel(settings: AppSettings, version: &str) -> AppSettings {
    let channel = channel_for(&settings, version).to_owned();
    AppSettings {
        update_channel: Some(channel),
        ..settings
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

/// Refuses an unknown `update_channel` (`settings.invalidChannel`); an
/// unset one is stored as the default for `version`.
fn save_app_settings_in(
    dir: &Path,
    incoming: AppSettings,
    version: &str,
) -> Result<AppSettings, String> {
    if let Some(c) = incoming.update_channel.as_deref() {
        if !UPDATE_CHANNELS.contains(&c) {
            return Err(ERR_INVALID_CHANNEL.to_string());
        }
    }
    let stored = load_app_settings(dir);
    let settings = with_channel(
        AppSettings {
            first_preview_approved: stored.first_preview_approved,
            ..incoming
        },
        version,
    );
    write_json(dir, SETTINGS_FILE, &settings, ERR_SETTINGS_IO)?;
    Ok(settings)
}

/// The stored settings with `update_channel` set (see
/// [`effective_update_channel`]).
#[tauri::command]
pub(crate) async fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, String> {
    let settings = load_app_settings(&config_dir(&app)?);
    let channel = effective_update_channel(&settings).to_owned();
    Ok(AppSettings {
        update_channel: Some(channel),
        ..settings
    })
}

/// Saves the settings the UI sends, except `first_preview_approved`, which
/// keeps its stored value. `update_channel` must be `"stable"` or `"beta"`
/// (`settings.invalidChannel` otherwise); when missing, the default for the
/// running version is stored. Returns the saved settings.
#[tauri::command]
pub(crate) async fn save_app_settings(
    settings: AppSettings,
    app: tauri::AppHandle,
) -> Result<AppSettings, String> {
    let saved = save_app_settings_in(&config_dir(&app)?, settings, APP_VERSION)?;
    log::info!("App settings saved");
    // The log level follows `debug_logging` without a restart.
    crate::logging::set_debug_logging(saved.debug_logging);
    // The tray menu follows the language setting.
    if let Err(e) = crate::setup::refresh_tray(&app) {
        log::warn!("Failed to update the tray menu: {e}");
    }
    Ok(saved)
}

// ---------------------------------------------------------------- approvals.json

/// A rule id with the rule version it had.
pub(crate) type RuleVersion = (Uuid, i32);

/// What executing a preview approves: `rules` (the rules the plan contains,
/// at their version) under `roots` (the watched folders the preview
/// planned).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PreviewApproval {
    pub(crate) rules: Vec<RuleVersion>,
    pub(crate) roots: Vec<PathBuf>,
}

/// Comparison key of a watched folder, Windows-style: `/` and `\` are the
/// same separator, a `\\?\` (or `\\?\UNC\`) prefix, `.` components,
/// repeated and trailing separators are dropped, and case is ignored. `..`
/// is kept as is. Pure string work, no disk access.
pub(crate) fn root_key(path: &Path) -> String {
    let s = path.to_string_lossy().replace('/', "\\");
    let s = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        s
    };
    let lead = if s.starts_with(r"\\") {
        r"\\"
    } else if s.starts_with('\\') {
        "\\"
    } else {
        ""
    };
    let parts: Vec<&str> = s
        .split('\\')
        .filter(|c| !c.is_empty() && *c != ".")
        .collect();
    format!("{lead}{}", parts.join("\\")).to_lowercase()
}

/// `key` is `root` or lies below it (both [`root_key`]s).
fn key_within(key: &str, root: &str) -> bool {
    key.strip_prefix(root)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('\\') || root.ends_with('\\'))
}

/// The approval of one rule: the version the user approved and the watched
/// folders ([`root_key`]s) the approving previews planned.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(from = "StoredApproval")]
pub(crate) struct Approval {
    pub(crate) version: i32,
    pub(crate) roots: BTreeSet<String>,
}

/// `approvals.json` entries as read: the current shape, or a bare version
/// from before approvals were per folder. A bare version approves no folder,
/// so those rules wait for the next preview.
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredApproval {
    PerFolder {
        version: i32,
        #[serde(default)]
        roots: BTreeSet<String>,
    },
    VersionOnly(i32),
}

impl From<StoredApproval> for Approval {
    fn from(s: StoredApproval) -> Self {
        match s {
            StoredApproval::PerFolder { version, roots } => Self { version, roots },
            StoredApproval::VersionOnly(version) => Self {
                version,
                roots: BTreeSet::new(),
            },
        }
    }
}

/// `approvals.json`: rule id -> the rule version the user approved by
/// executing a preview that contained the rule, and the watched folders that
/// preview (or an earlier one at the same version) planned. Auto-apply runs a
/// rule only at its approved version and only on files under an approved
/// folder: editing a rule bumps its version, and adding a watched folder
/// (also re-adding a removed one) needs a preview of that folder.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(transparent)]
pub(crate) struct Approvals(pub(crate) BTreeMap<Uuid, Approval>);

impl Approvals {
    /// `rule` is approved at its current version under the watched folder
    /// whose [`root_key`] is `root`.
    pub(crate) fn allows(&self, rule: &Rule, root: &str) -> bool {
        self.0
            .get(&rule.id)
            .is_some_and(|a| a.version == rule.version && a.roots.contains(root))
    }

    /// `rule` may run on `path` in a pass over `roots`: approved at its
    /// current version under every one of `roots` that contains `path`
    /// (watched folders may nest), and at least one contains it.
    pub(crate) fn allows_path(&self, rule: &Rule, path: &Path, roots: &[String]) -> bool {
        let key = root_key(path);
        let mut containing = roots.iter().filter(|r| key_within(&key, r)).peekable();
        containing.peek().is_some() && containing.all(|r| self.allows(rule, r))
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

/// Records that the user executed a preview: each rule of `approval` is
/// approved at its version under the preview's folders (added to the
/// folders of an approval at the same version, replacing an approval at
/// another version), and `first_preview_approved` is set in
/// `app_settings.json`.
pub(crate) fn record_preview_approval(
    dir: &Path,
    approval: &PreviewApproval,
) -> Result<(), String> {
    let mut approvals = load_approvals(dir);
    let roots: BTreeSet<String> = approval.roots.iter().map(|r| root_key(r)).collect();
    for (id, version) in &approval.rules {
        match approvals.0.get_mut(id) {
            Some(a) if a.version == *version => a.roots.extend(roots.iter().cloned()),
            _ => {
                approvals.0.insert(
                    *id,
                    Approval {
                        version: *version,
                        roots: roots.clone(),
                    },
                );
            }
        }
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

/// Drops from every approval the folders that are not in `watched` any more
/// (and approvals left without a folder), so a folder removed and added
/// again needs a new preview. Folders still watched keep their approvals.
/// Leaves the file alone when nothing changes.
pub(crate) fn prune_approval_roots(dir: &Path, watched: &[String]) -> Result<(), String> {
    let keep: BTreeSet<String> = watched.iter().map(|w| root_key(Path::new(w))).collect();
    let mut approvals = load_approvals(dir);
    let before = approvals.clone();
    for a in approvals.0.values_mut() {
        a.roots.retain(|r| keep.contains(r));
    }
    approvals.0.retain(|_, a| !a.roots.is_empty());
    if approvals != before {
        write_json(dir, APPROVALS_FILE, &approvals, ERR_SETTINGS_IO)?;
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

    /// A plan without folder or rule limits.
    const FULL: PlanTier = PlanTier::Trial { ends_at: i64::MAX };

    #[test]
    fn remove_approval_drops_only_that_rule() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        remove_approval(dir, Uuid::from_u128(1)).unwrap();
        assert!(!dir.join(APPROVALS_FILE).exists(), "nothing to write");
        approve(dir, &[(1, 2), (2, 1)], &[IN]);
        remove_approval(dir, Uuid::from_u128(1)).unwrap();
        let a = load_approvals(dir);
        assert!(!a.allows(&rule(1, 2), &key(IN)));
        assert!(a.allows(&rule(2, 1), &key(IN)));
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
        assert_eq!(validate_config(&cfg(&["C:\\Users\\Ana"])), Ok(()));
        assert_eq!(
            validate_config(&cfg(&["C:\\Users\\Ana", "D:\\"])),
            Err("config.driveRoot: D:\\".to_string())
        );
        assert_eq!(
            validate_config(&cfg(&["\\\\nas\\share"])),
            Err("config.driveRoot: \\\\nas\\share".to_string())
        );
        assert_eq!(
            validate_config(&cfg(&["C:\\Users\\Ana\\AppData\\Local"])),
            Err("config.excludedFolder: C:\\Users\\Ana\\AppData\\Local".to_string())
        );
        // A folder that does not exist (unplugged drive) is not refused.
        assert_eq!(validate_config(&cfg(&["E:\\Backup\\in"])), Ok(()));
    }

    #[test]
    fn save_config_writes_only_valid_config_and_returns_it() {
        // The system temp dir is under AppData on Windows, which save_config
        // refuses, so the crate folder stands in for an existing watched
        // folder. It is only checked for existence, never written.
        let watched = Path::new(env!("CARGO_MANIFEST_DIR"));
        let w = watched.to_str().unwrap();
        assert!(
            !in_excluded_folder(w),
            "checkout inside an excluded folder: {w}"
        );
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("cfg");
        let ok = cfg(&[w]);
        assert_eq!(save_config_in(&dir, ok.clone(), &FULL).unwrap(), ok);
        assert_eq!(read_config(&dir).unwrap(), ok);

        let CmdError::Code(err) = save_config_in(&dir, cfg(&[w, "C:\\"]), &FULL).unwrap_err()
        else {
            panic!("not a code");
        };
        assert!(err.starts_with("config.driveRoot: "), "{err}");
        assert_eq!(read_config(&dir).unwrap(), ok, "unchanged after a refusal");

        // A missing folder is saved and reported as unavailable.
        let missing = watched.join("valet-test-missing");
        let m = missing.to_str().unwrap();
        let with_missing = cfg(&[w, m]);
        assert_eq!(
            save_config_in(&dir, with_missing.clone(), &FULL).unwrap(),
            with_missing
        );
        assert_eq!(read_config(&dir).unwrap(), with_missing);
        let view = config_view(&dir, Path::exists, None).unwrap();
        assert!(view.invalid_paths.is_empty());
        assert_eq!(view.unavailable_paths, [m]);
        assert_eq!(
            watched_folders(&dir, None).unwrap(),
            vec![watched.to_path_buf()]
        );
        // Only missing folders: saved, nothing to watch or plan.
        save_config_in(&dir, cfg(&[m]), &FULL).unwrap();
        assert_eq!(watched_folders(&dir, None).unwrap_err(), ERR_NO_FOLDERS);
    }

    #[test]
    fn free_plan_refuses_a_third_folder_but_allows_removing_and_reordering() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let free = PlanTier::Free;
        let (a, b, c) = (
            "C:\\Users\\Ana\\A",
            "C:\\Users\\Ana\\B",
            "C:\\Users\\Ana\\C",
        );
        save_config_in(dir, cfg(&[a, b]), &free).unwrap();
        let err = save_config_in(dir, cfg(&[a, b, c]), &free).unwrap_err();
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            json!({ "code": "plan.limit", "kind": "folders", "limit": 2, "detail": "folders 2" })
        );
        assert_eq!(read_config(dir).unwrap(), cfg(&[a, b]), "unchanged");
        // Replacing one folder by another also adds one: allowed within 2.
        save_config_in(dir, cfg(&[b, c]), &free).unwrap();
        // Trial: three folders.
        save_config_in(dir, cfg(&[a, b, c]), &FULL).unwrap();
        // Back on Free: reordering and removing are allowed with 3 stored.
        save_config_in(dir, cfg(&[c, a, b]), &free).unwrap();
        save_config_in(dir, cfg(&[c, a]), &free).unwrap();
        assert_eq!(read_config(dir).unwrap(), cfg(&[c, a]));
    }

    #[test]
    fn folders_beyond_the_free_limit_are_inactive_and_not_watched() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let (a, b, c, d) = (
            "C:\\Users\\Ana\\A",
            "C:\\Users\\Ana\\B",
            "C:\\Users\\Ana\\C",
            "C:\\Users\\Ana\\D",
        );
        save_config_in(dir, cfg(&[a, b, c, d]), &FULL).unwrap();
        let exists = |p: &Path| p != Path::new(b);
        let limit = folder_limit(&PlanTier::Free);
        assert_eq!(limit, Some(2));
        let view = config_view(dir, exists, limit).unwrap();
        assert_eq!(view.config, cfg(&[a, b, c, d]), "all entries kept");
        assert_eq!(view.unavailable_paths, [b]);
        assert_eq!(view.inactive_paths, [c, d]);
        assert_eq!(
            watched_folders_in(dir, exists, limit).unwrap(),
            vec![PathBuf::from(a)]
        );
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["inactive_paths"], json!([c, d]));
        // Trial or Pro: everything active again.
        let view = config_view(dir, exists, folder_limit(&FULL)).unwrap();
        assert!(view.inactive_paths.is_empty());
        assert_eq!(watched_folders_in(dir, exists, None).unwrap().len(), 3);
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

    /// An existing folder outside the system temp dir (under AppData on
    /// Windows, which validation refuses): the crate folder. Only checked
    /// for existence, never written.
    fn existing_folder() -> PathBuf {
        let w = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert!(!in_excluded_folder(w.to_str().unwrap()), "{}", w.display());
        w
    }

    #[test]
    fn watched_folders_without_existing_folders_is_no_folders() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(
            watched_folders(tmp.path(), None).unwrap_err(),
            ERR_NO_FOLDERS
        );
        let dir = existing_folder();
        let gone = dir.join("valet-test-missing");
        write_json(
            tmp.path(),
            CONFIG_FILE,
            &cfg(&[gone.to_str().unwrap()]),
            ERR_CONFIG_IO,
        )
        .unwrap();
        assert_eq!(
            watched_folders(tmp.path(), None).unwrap_err(),
            ERR_NO_FOLDERS
        );
        write_json(
            tmp.path(),
            CONFIG_FILE,
            &cfg(&[gone.to_str().unwrap(), dir.to_str().unwrap()]),
            ERR_CONFIG_IO,
        )
        .unwrap();
        assert_eq!(watched_folders(tmp.path(), None).unwrap(), vec![dir]);
    }

    #[test]
    fn split_config_applies_the_save_config_checks() {
        let present = [
            "C:\\Users\\Ana\\Downloads",
            "D:\\",
            "C:\\Users\\Ana\\AppData\\x",
        ];
        let exists = |p: &Path| present.iter().any(|q| Path::new(q) == p);
        let c = cfg(&[
            "D:\\",
            "C:\\Users\\Ana\\Downloads",
            "C:\\Users\\Ana\\AppData\\x",
            "C:\\Users\\Ana\\Gone",
            "\\\\nas\\share",
        ]);
        let split = split_config(&c, exists, None);
        assert_eq!(
            split.available,
            vec![PathBuf::from("C:\\Users\\Ana\\Downloads")]
        );
        let got: Vec<(&str, &str)> = split
            .invalid
            .iter()
            .map(|i| (i.path.as_str(), i.code))
            .collect();
        assert_eq!(
            got,
            [
                ("D:\\", ERR_DRIVE_ROOT),
                ("C:\\Users\\Ana\\AppData\\x", ERR_EXCLUDED_FOLDER),
                ("\\\\nas\\share", ERR_DRIVE_ROOT),
            ]
        );
        assert_eq!(split.unavailable, ["C:\\Users\\Ana\\Gone"]);
        // Every entry save_config accepts is valid here, and only those.
        for path in &c.inbox_paths {
            let one = cfg(&[path]);
            assert_eq!(
                validate_config(&one).is_ok(),
                split_config(&one, exists, None).invalid.is_empty(),
                "{path}"
            );
        }
    }

    #[test]
    fn hand_edited_config_is_validated_on_load_and_kept_in_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = existing_folder();
        let excluded = dir.join("node_modules");
        let gone = dir.join("valet-test-missing");
        // "/" exists on Linux, "C:\\" on Windows: refused even though present.
        let entries = [
            "/",
            "C:\\",
            excluded.to_str().unwrap(),
            gone.to_str().unwrap(),
            dir.to_str().unwrap(),
        ];
        write_json(tmp.path(), CONFIG_FILE, &cfg(&entries), ERR_CONFIG_IO).unwrap();
        let on_disk = std::fs::read(tmp.path().join(CONFIG_FILE)).unwrap();

        assert_eq!(
            watched_folders(tmp.path(), None).unwrap(),
            vec![dir.clone()]
        );

        let view = config_view(tmp.path(), Path::exists, None).unwrap();
        assert_eq!(view.config, cfg(&entries), "nothing dropped");
        assert_eq!(
            serde_json::to_value(&view).unwrap(),
            json!({
                "inbox_paths": entries,
                "invalid_paths": [
                    { "path": "/", "code": "config.driveRoot" },
                    { "path": "C:\\", "code": "config.driveRoot" },
                    { "path": excluded.to_str().unwrap(), "code": "config.excludedFolder" },
                ],
                "unavailable_paths": [gone.to_str().unwrap()],
                "inactive_paths": [],
            })
        );
        assert_eq!(
            std::fs::read(tmp.path().join(CONFIG_FILE)).unwrap(),
            on_disk
        );

        // Only invalid entries: no folder to watch or plan.
        write_json(tmp.path(), CONFIG_FILE, &cfg(&entries[..4]), ERR_CONFIG_IO).unwrap();
        assert_eq!(
            watched_folders(tmp.path(), None).unwrap_err(),
            ERR_NO_FOLDERS
        );
        let empty = tmp.path().join("none");
        assert_eq!(
            config_view(&empty, Path::exists, None).unwrap(),
            ConfigView {
                config: Config::default(),
                invalid_paths: vec![],
                unavailable_paths: vec![],
                inactive_paths: vec![],
            }
        );
    }

    #[test]
    fn unavailable_folder_is_picked_up_again_when_it_reappears() {
        let tmp = tempfile::tempdir().unwrap();
        let usb = "E:\\Photos";
        let home = "C:\\Users\\Ana\\Downloads";
        write_json(tmp.path(), CONFIG_FILE, &cfg(&[usb, home]), ERR_CONFIG_IO).unwrap();
        let plugged = std::cell::Cell::new(false);
        let exists = |p: &Path| p == Path::new(home) || (plugged.get() && p == Path::new(usb));

        assert_eq!(
            watched_folders_in(tmp.path(), exists, None).unwrap(),
            vec![PathBuf::from(home)]
        );
        let view = config_view(tmp.path(), exists, None).unwrap();
        assert_eq!(view.unavailable_paths, [usb]);
        assert!(view.invalid_paths.is_empty());

        plugged.set(true);
        assert_eq!(
            watched_folders_in(tmp.path(), exists, None).unwrap(),
            vec![PathBuf::from(usb), PathBuf::from(home)],
            "configuration order, no restart or save needed"
        );
        assert!(config_view(tmp.path(), exists, None)
            .unwrap()
            .unavailable_paths
            .is_empty());
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
                "debug_logging": false,
                "auto_update_check": true,
            })
        );
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(load_app_settings(tmp.path()), s, "missing file");
    }

    #[test]
    fn update_channel_defaults_to_the_app_version_kind_until_stored() {
        let unset = AppSettings::default();
        assert_eq!(channel_for(&unset, "0.3.0"), "stable");
        assert_eq!(channel_for(&unset, "0.3.0-beta.1"), "beta");
        let stored = AppSettings {
            update_channel: Some("stable".into()),
            ..AppSettings::default()
        };
        assert_eq!(channel_for(&stored, "0.3.0-beta.1"), "stable");
        assert_eq!(
            effective_update_channel(&unset),
            if APP_VERSION.contains('-') {
                "beta"
            } else {
                "stable"
            }
        );
        assert_eq!(effective_update_channel(&stored), "stable");
    }

    #[test]
    fn saving_stores_the_channel_explicitly_and_refuses_unknown_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // Saved once on a beta build: stays beta after a stable release.
        let saved = save_app_settings_in(dir, AppSettings::default(), "0.3.0-beta.1").unwrap();
        assert_eq!(saved.update_channel.as_deref(), Some("beta"));
        let loaded = load_app_settings(dir);
        assert_eq!(loaded, saved);
        assert_eq!(channel_for(&loaded, "1.0.0"), "beta");

        // The user picks stable.
        let ui = AppSettings {
            update_channel: Some("stable".into()),
            debug_logging: true,
            ..loaded
        };
        let saved = save_app_settings_in(dir, ui, "0.3.0-beta.1").unwrap();
        assert_eq!(saved.update_channel.as_deref(), Some("stable"));
        assert!(saved.debug_logging);
        assert_eq!(load_app_settings(dir), saved);

        // Unknown channel: refused, nothing written.
        let before = std::fs::read(dir.join(SETTINGS_FILE)).unwrap();
        for bad in ["nightly", "", "Beta"] {
            let err = save_app_settings_in(
                dir,
                AppSettings {
                    update_channel: Some(bad.into()),
                    ..AppSettings::default()
                },
                "0.3.0",
            )
            .unwrap_err();
            assert_eq!(err, ERR_INVALID_CHANNEL, "{bad:?}");
        }
        assert_eq!(std::fs::read(dir.join(SETTINGS_FILE)).unwrap(), before);
    }

    #[test]
    fn unknown_stored_channel_loads_as_unset() {
        let tmp = tempfile::tempdir().unwrap();
        for (stored, want) in [
            (json!("nightly"), None),
            (json!(3), None),
            (json!(null), None),
            (json!("beta"), Some("beta")),
            (json!("stable"), Some("stable")),
        ] {
            std::fs::write(
                tmp.path().join(SETTINGS_FILE),
                json!({ "update_channel": stored, "debug_logging": true }).to_string(),
            )
            .unwrap();
            let s = load_app_settings(tmp.path());
            assert_eq!(s.update_channel.as_deref(), want, "{stored}");
            assert!(s.debug_logging);
        }
        // get_app_settings fills in the default.
        let s = with_channel(AppSettings::default(), "0.3.0-rc.1");
        assert_eq!(s.update_channel.as_deref(), Some("beta"));
    }

    #[test]
    fn auto_update_check_defaults_to_on_and_is_saved() {
        let tmp = tempfile::tempdir().unwrap();
        // A settings file from before the field.
        std::fs::write(
            tmp.path().join(SETTINGS_FILE),
            r#"{"language":"en","onboarding_done":true,"update_channel":"stable"}"#,
        )
        .unwrap();
        assert!(load_app_settings(tmp.path()).auto_update_check);
        // The UI turns it off.
        let off = AppSettings {
            auto_update_check: false,
            ..load_app_settings(tmp.path())
        };
        let saved = save_app_settings_in(tmp.path(), off, "0.3.0").unwrap();
        assert!(!saved.auto_update_check);
        assert!(!load_app_settings(tmp.path()).auto_update_check);
        // A UI that does not send the field keeps the default (on).
        let from_ui: AppSettings = serde_json::from_value(json!({ "language": "ro" })).unwrap();
        assert!(from_ui.auto_update_check);
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

        approve(dir, &[(1, 2)], &[IN]);
        let a = load_approvals(dir);
        let k = key(IN);
        assert!(a.allows(&rule(1, 2), &k));
        assert!(
            !a.allows(&rule(1, 3), &k),
            "an edited rule needs a new preview"
        );
        assert!(!a.allows(&rule(1, 1), &k));
        assert!(!a.allows(&rule(2, 2), &k), "never previewed");
        assert!(load_app_settings(dir).first_preview_approved);

        // A later preview approves the new version and keeps other rules.
        approve(dir, &[(1, 3), (2, 1)], &[IN]);
        let a = load_approvals(dir);
        assert!(a.allows(&rule(1, 3), &k) && !a.allows(&rule(1, 2), &k));
        assert!(a.allows(&rule(2, 1), &k));

        // An unreadable file approves nothing.
        std::fs::write(dir.join(APPROVALS_FILE), "{ nope").unwrap();
        assert_eq!(load_approvals(dir), Approvals::default());
    }

    const IN: &str = "C:\\Users\\Ana\\Downloads";
    const DESK: &str = "C:\\Users\\Ana\\Desktop";

    fn key(p: &str) -> String {
        root_key(Path::new(p))
    }

    /// Executing a preview of `roots` that contained `rules` (n, version).
    fn approve(dir: &Path, rules: &[(u128, i32)], roots: &[&str]) {
        let approval = PreviewApproval {
            rules: rules
                .iter()
                .map(|(n, v)| (Uuid::from_u128(*n), *v))
                .collect(),
            roots: roots.iter().map(PathBuf::from).collect(),
        };
        record_preview_approval(dir, &approval).unwrap();
    }

    #[test]
    fn root_key_ignores_case_separators_and_long_path_prefix() {
        let k = key("C:\\Users\\Ana\\Downloads");
        for same in [
            "c:\\users\\ana\\downloads",
            "C:/Users/Ana/Downloads",
            "C:\\Users\\Ana\\Downloads\\",
            "C:\\Users\\\\Ana\\.\\Downloads",
            "\\\\?\\C:\\Users\\Ana\\Downloads",
        ] {
            assert_eq!(key(same), k, "{same}");
        }
        assert_ne!(key("C:\\Users\\Ana\\Downloads2"), k);
        assert_ne!(key("D:\\Users\\Ana\\Downloads"), k);
        let unc = key("\\\\nas\\share\\in");
        assert_eq!(unc, "\\\\nas\\share\\in");
        assert_eq!(key("\\\\?\\UNC\\NAS\\share\\in\\"), unc);
        assert_eq!(key("//nas/share/in"), unc);
        assert_eq!(key("/home/Ana/in/"), "\\home\\ana\\in");
    }

    #[test]
    fn approval_is_per_watched_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        approve(dir, &[(1, 1)], &[IN]);
        let a = load_approvals(dir);
        assert!(a.allows(&rule(1, 1), &key(IN)));
        assert!(
            a.allows(&rule(1, 1), &key("c:/users/ana/downloads/")),
            "same folder, other spelling"
        );
        assert!(!a.allows(&rule(1, 1), &key(DESK)), "never previewed there");

        // Another preview at the same version adds its folders.
        approve(dir, &[(1, 1)], &[DESK]);
        let a = load_approvals(dir);
        assert!(a.allows(&rule(1, 1), &key(IN)) && a.allows(&rule(1, 1), &key(DESK)));

        // A new version starts over with the folders of its preview.
        approve(dir, &[(1, 2)], &[DESK]);
        let a = load_approvals(dir);
        assert!(a.allows(&rule(1, 2), &key(DESK)));
        assert!(!a.allows(&rule(1, 2), &key(IN)));

        // Stored shape.
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            json!({ Uuid::from_u128(1).to_string(): { "version": 2, "roots": [key(DESK)] } })
        );
    }

    #[test]
    fn version_only_approvals_from_before_approve_no_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(
            dir.join(APPROVALS_FILE),
            json!({ Uuid::from_u128(1).to_string(): 2 }).to_string(),
        )
        .unwrap();
        let a = load_approvals(dir);
        assert_eq!(a.0[&Uuid::from_u128(1)].version, 2);
        assert!(!a.allows(&rule(1, 2), &key(IN)), "needs a new preview");
        // The next preview turns it into a per-folder approval.
        approve(dir, &[(1, 2)], &[IN]);
        assert!(load_approvals(dir).allows(&rule(1, 2), &key(IN)));
    }

    #[test]
    fn removing_a_folder_keeps_the_others_and_re_adding_it_needs_a_preview() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        approve(dir, &[(1, 1), (2, 1)], &[IN, DESK]);
        approve(dir, &[(3, 1)], &[DESK]);
        let before = std::fs::read(dir.join(APPROVALS_FILE)).unwrap();
        prune_approval_roots(dir, &[IN.to_string(), DESK.to_string()]).unwrap();
        assert_eq!(
            std::fs::read(dir.join(APPROVALS_FILE)).unwrap(),
            before,
            "nothing removed"
        );

        // The desktop is no longer watched (spelled differently in config).
        prune_approval_roots(dir, &["c:/users/ana/downloads/".to_string()]).unwrap();
        let a = load_approvals(dir);
        assert!(a.allows(&rule(1, 1), &key(IN)), "no new approval needed");
        assert!(a.allows(&rule(2, 1), &key(IN)));
        assert!(
            !a.allows(&rule(1, 1), &key(DESK)),
            "re-added: preview again"
        );
        assert!(!a.0.contains_key(&Uuid::from_u128(3)), "no folder left");

        // No approvals file: nothing written.
        let empty = dir.join("none");
        prune_approval_roots(&empty, &[]).unwrap();
        assert!(!empty.join(APPROVALS_FILE).exists());
    }

    #[test]
    fn save_config_drops_approvals_of_removed_folders() {
        let watched = existing_folder();
        let w = watched.to_str().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        approve(dir, &[(1, 1)], &[w, IN]);
        save_config_in(dir, cfg(&[w]), &FULL).unwrap();
        let a = load_approvals(dir);
        assert!(a.allows(&rule(1, 1), &key(w)));
        assert!(!a.allows(&rule(1, 1), &key(IN)));

        // A folder that is unavailable when saving keeps its approvals.
        let usb = existing_folder().join("valet-test-missing");
        let u = usb.to_str().unwrap();
        approve(dir, &[(1, 1)], &[u]);
        save_config_in(dir, cfg(&[w, u]), &FULL).unwrap();
        let a = load_approvals(dir);
        assert!(a.allows(&rule(1, 1), &key(u)), "kept while unavailable");
        assert!(a.allows(&rule(1, 1), &key(w)));

        // A refused config changes no approval.
        approve(dir, &[(1, 1)], &[IN]);
        let before = load_approvals(dir);
        save_config_in(dir, cfg(&["C:\\"]), &FULL).unwrap_err();
        assert_eq!(load_approvals(dir), before);
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
        let saved = save_app_settings_in(dir, ui.clone(), "0.1.0").unwrap();
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
            "0.1.0",
        )
        .unwrap();
        assert!(saved.first_preview_approved);
    }
}
