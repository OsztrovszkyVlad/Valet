//! Builds the plan: which file goes where under which rule, and why every
//! other file in scope was left alone. The planner lists directories and reads
//! metadata; it never opens, hashes, moves or writes a file.

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Datelike, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    collision::next_free_name,
    matcher::{FileFacts, rule_matches},
    platform::Platform,
    rules::{Action, Rule},
};

/// Directory names never descended below a scan root (ASCII case-insensitive).
pub const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    "Windows",
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
    "AppData",
    "$Recycle.Bin",
    "System Volume Information",
    ".git",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "__pycache__",
];

/// A directory containing a file with this name is excluded with everything under it.
pub const IGNORE_MARKER: &str = ".valetignore";

/// Extensions of files that are still being downloaded or written (lowercase).
const BEING_WRITTEN_EXTS: &[&str] = &["crdownload", "part", "tmp", "download", "partial"];

/// Files modified more recently than this are treated as still being written.
pub const RECENT_WRITE_WINDOW: Duration = Duration::from_secs(10);

/// Environment variable lookup (`None` when unset).
pub type EnvLookup = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Inputs from the environment, injected so plans are deterministic in tests.
pub struct PlanContext {
    pub now: SystemTime,
    pub home_dir: Option<PathBuf>,
    pub env: EnvLookup,
}

impl PlanContext {
    /// Current time, the user's home directory and the process environment.
    pub fn from_system() -> Self {
        Self {
            now: SystemTime::now(),
            home_dir: directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()),
            env: Box::new(|key| std::env::var(key).ok()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OpKind {
    Move,
    Copy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    Excluded,
    Hidden,
    ReparsePoint,
    BeingWritten,
    CloudPlaceholder,
    Conflict,
    AlreadyInPlace,
    NoExtension,
    /// No usable destination folder: missing or empty path, unset `~` or
    /// `%VAR%`, a folder inside a default-excluded directory (outside the
    /// file's own scan root), or a folder at or under a `.valetignore` marker.
    InvalidDestination,
    UnsupportedAction,
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOp {
    pub source: PathBuf,
    /// Full destination file path, collision-free at planning time.
    pub dest: PathBuf,
    pub kind: OpKind,
    pub rule_id: Uuid,
    pub rule_name: String,
    pub size: u64,
    pub mtime: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skipped {
    pub path: PathBuf,
    pub reason: SkipReason,
    pub rule_name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub ops: Vec<PlannedOp>,
    pub skipped: Vec<Skipped>,
}

/// Plans at most one move or copy per file found under `roots`.
///
/// `rules` are evaluated in the given order (the order of `Db::list_rules`);
/// the first enabled matching rule with a `moveTo`/`copyTo` action wins.
pub fn plan(roots: &[PathBuf], rules: &[Rule], platform: &dyn Platform, ctx: &PlanContext) -> Plan {
    let mut out = Plan::default();
    let mut reserved: HashSet<String> = HashSet::new();
    let mut marker_cache: HashMap<PathBuf, bool> = HashMap::new();
    scan(roots, platform, ctx.now, |found| match found {
        Found::Skip(s) => out.skipped.push(s),
        Found::File(f) => plan_file(
            &f,
            rules,
            platform,
            ctx,
            &mut reserved,
            &mut marker_cache,
            &mut out,
        ),
    });
    out
}

// ---------------------------------------------------------------- walker

/// A regular file that passed the walker's checks.
pub(crate) struct FoundFile {
    pub path: PathBuf,
    /// Folder that relative destinations resolve against (the scan root,
    /// or its parent when the root is a file).
    pub base: PathBuf,
    pub size: u64,
    pub mtime: SystemTime,
    /// Dot-name or hidden/system attribute; the caller decides.
    pub hidden: bool,
}

pub(crate) enum Found {
    File(FoundFile),
    Skip(Skipped),
}

fn skip(path: PathBuf, reason: SkipReason) -> Found {
    Found::Skip(Skipped {
        path,
        reason,
        rule_name: None,
    })
}

/// Walks `roots` in order with the default exclusions. Emits each file once
/// (overlapping roots are deduplicated) and one `Skip` per excluded,
/// hidden, reparse-point, placeholder or unreadable entry. Excluded
/// directories are not descended.
pub(crate) fn scan(
    roots: &[PathBuf],
    platform: &dyn Platform,
    now: SystemTime,
    mut emit: impl FnMut(Found),
) {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    // A directory the user chose as a root is never excluded, also when it
    // lies inside another root.
    let explicit: HashSet<PathBuf> = roots.iter().map(|r| normalize(r)).collect();
    for root in roots {
        let root = normalize(root);
        let os_root = platform.os_path(&root);
        if fs::symlink_metadata(&os_root).is_err() {
            if seen.insert(root.clone()) {
                emit(skip(root, SkipReason::Unreadable));
            }
            continue;
        }
        let root_is_dir = fs::metadata(&os_root).map(|m| m.is_dir()).unwrap_or(false);
        let base = if root_is_dir {
            root.clone()
        } else {
            root.parent().map(Path::to_path_buf).unwrap_or_default()
        };
        if root_is_dir && has_ignore_marker(platform, &root) {
            if seen.insert(root.clone()) {
                emit(skip(root, SkipReason::Excluded));
            }
            continue;
        }

        let logical = |p: &Path| -> PathBuf {
            match p.strip_prefix(&os_root) {
                Ok(rel) if rel.as_os_str().is_empty() => root.clone(),
                Ok(rel) => root.join(rel),
                Err(_) => p.to_path_buf(),
            }
        };

        let mut it = WalkDir::new(&os_root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter();
        while let Some(res) = it.next() {
            let entry = match res {
                Ok(e) => e,
                Err(err) => {
                    let p = err.path().map(logical).unwrap_or_else(|| root.clone());
                    if seen.insert(p.clone()) {
                        emit(skip(p, SkipReason::Unreadable));
                    }
                    continue;
                }
            };
            let path = logical(entry.path());
            let is_root = entry.depth() == 0;
            let ft = entry.file_type();
            let is_dir = ft.is_dir();
            let explicit_dir = is_dir && (is_root || explicit.contains(&path));

            if seen.contains(&path) {
                if is_dir {
                    it.skip_current_dir();
                }
                continue;
            }

            let reason = if platform.is_cloud_placeholder(&path) {
                Some(SkipReason::CloudPlaceholder)
            } else if !explicit_dir && (entry.path_is_symlink() || platform.is_reparse_point(&path))
            {
                Some(SkipReason::ReparsePoint)
            } else if is_dir && !is_root {
                if explicit_dir {
                    // Chosen as a root elsewhere: only the user's own marker excludes it.
                    has_ignore_marker(platform, &path).then_some(SkipReason::Excluded)
                } else {
                    dir_skip_reason(platform, &path, entry.file_name())
                }
            } else {
                None
            };
            if let Some(reason) = reason {
                if is_dir {
                    it.skip_current_dir();
                }
                seen.insert(path.clone());
                emit(skip(path, reason));
                continue;
            }
            if !ft.is_file() {
                // Directories that are walked, and special files (sockets, devices).
                continue;
            }
            seen.insert(path.clone());

            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => {
                    emit(skip(path, SkipReason::Unreadable));
                    continue;
                }
            };
            let Ok(mtime) = meta.modified() else {
                emit(skip(path, SkipReason::Unreadable));
                continue;
            };
            if is_being_written(entry.file_name(), mtime, now) {
                emit(skip(path, SkipReason::BeingWritten));
                continue;
            }
            let hidden =
                !is_root && (is_dot_name(entry.file_name()) || platform.is_hidden_or_system(&path));
            emit(Found::File(FoundFile {
                path,
                base: base.clone(),
                size: meta.len(),
                mtime,
                hidden,
            }));
        }
    }
}

fn dir_skip_reason(platform: &dyn Platform, path: &Path, name: &OsStr) -> Option<SkipReason> {
    let name_str = name.to_str().unwrap_or("");
    if DEFAULT_EXCLUDED_DIRS
        .iter()
        .any(|ex| ex.eq_ignore_ascii_case(name_str))
        || has_ignore_marker(platform, path)
    {
        Some(SkipReason::Excluded)
    } else if is_dot_name(name) || platform.is_hidden_or_system(path) {
        Some(SkipReason::Hidden)
    } else {
        None
    }
}

fn has_ignore_marker(platform: &dyn Platform, dir: &Path) -> bool {
    fs::symlink_metadata(platform.os_path(&dir.join(IGNORE_MARKER))).is_ok()
}

fn is_dot_name(name: &OsStr) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

fn is_being_written(name: &OsStr, mtime: SystemTime, now: SystemTime) -> bool {
    let p = Path::new(name);
    let ext_hit = p
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|e| BEING_WRITTEN_EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)));
    let office_lock = name.as_encoded_bytes().starts_with(b"~$");
    let recent = match now.checked_sub(RECENT_WRITE_WINDOW) {
        Some(cutoff) => mtime > cutoff,
        None => true,
    };
    ext_hit || office_lock || recent
}

/// Lexical normalization: drops `.` components and resolves `..` against the
/// preceding normal component. Does not touch the disk.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else {
                    out.push(c);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Case-insensitive key for comparing paths (Windows file systems ignore case).
fn fold(p: &Path) -> String {
    normalize(p).to_string_lossy().to_lowercase()
}

// ---------------------------------------------------------------- per-file decision

fn first_transfer(rule: &Rule) -> Option<(OpKind, &Action)> {
    rule.actions.iter().find_map(|a| match a.r#type.as_str() {
        "moveTo" => Some((OpKind::Move, a)),
        "copyTo" => Some((OpKind::Copy, a)),
        _ => None,
    })
}

fn has_unsupported_action(rule: &Rule) -> bool {
    rule.actions
        .iter()
        .any(|a| !matches!(a.r#type.as_str(), "moveTo" | "copyTo"))
}

fn include_hidden(rule: &Rule) -> bool {
    rule.options
        .get("includeHidden")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

fn plan_file(
    f: &FoundFile,
    rules: &[Rule],
    platform: &dyn Platform,
    ctx: &PlanContext,
    reserved: &mut HashSet<String>,
    marker_cache: &mut HashMap<PathBuf, bool>,
    out: &mut Plan,
) {
    let path_str = f.path.to_string_lossy();
    let facts = FileFacts {
        path: &path_str,
        size: f.size,
    };
    let matching: Vec<&Rule> = rules.iter().filter(|r| rule_matches(r, &facts)).collect();
    let winner = matching.iter().position(|r| first_transfer(r).is_some());

    let push_skip = |out: &mut Plan, reason, rule: Option<&Rule>| {
        out.skipped.push(Skipped {
            path: f.path.clone(),
            reason,
            rule_name: rule.map(|r| r.name.clone()),
        });
    };

    if f.hidden {
        let decider = winner.map(|i| matching[i]).or(matching.first().copied());
        if !decider.is_some_and(include_hidden) {
            push_skip(out, SkipReason::Hidden, decider);
            return;
        }
    }

    let mut planned = false;
    if let Some(wi) = winner {
        let rule = matching[wi];
        let (kind, action) = first_transfer(rule).expect("winner has a transfer action");
        match destination(f, action, platform, ctx, reserved, marker_cache) {
            Ok(dest) => {
                out.ops.push(PlannedOp {
                    source: f.path.clone(),
                    dest,
                    kind,
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    size: f.size,
                    mtime: f.mtime,
                });
                planned = true;
            }
            Err(reason) => push_skip(out, reason, Some(rule)),
        }
        for later in &matching[wi + 1..] {
            if first_transfer(later).is_some() {
                push_skip(out, SkipReason::Conflict, Some(later));
            }
        }
    }
    if !planned {
        for rule in matching.iter().filter(|r| has_unsupported_action(r)) {
            push_skip(out, SkipReason::UnsupportedAction, Some(rule));
        }
    }
}

/// Full collision-free destination path for `f` under the folder template of
/// `action.params.path`. Reserves the result in `reserved`.
fn destination(
    f: &FoundFile,
    action: &Action,
    platform: &dyn Platform,
    ctx: &PlanContext,
    reserved: &mut HashSet<String>,
    marker_cache: &mut HashMap<PathBuf, bool>,
) -> Result<PathBuf, SkipReason> {
    let template = action
        .params
        .get("path")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or(SkipReason::InvalidDestination)?;
    let folder = expand_folder(template, f, ctx)?;
    let folder = if folder.is_absolute() {
        folder
    } else {
        f.base.join(folder)
    };
    let folder = normalize(&folder);
    if in_excluded_dir(&folder, &f.base) || under_ignore_marker(platform, &folder, marker_cache) {
        return Err(SkipReason::InvalidDestination);
    }
    let file_name = f.path.file_name().ok_or(SkipReason::InvalidDestination)?;
    if f.path.parent().is_some_and(|p| fold(p) == fold(&folder)) {
        return Err(SkipReason::AlreadyInPlace);
    }
    let dest = next_free_name(&folder.join(file_name), |p| {
        reserved.contains(&fold(p)) || fs::symlink_metadata(platform.os_path(p)).is_ok()
    });
    reserved.insert(fold(&dest));
    Ok(dest)
}

/// True when a component of `folder` is a default-excluded directory name
/// (ASCII case-insensitive). Leading components shared with `base` (the scan
/// root the user chose) are not checked: the scan does not exclude them either,
/// and on Windows temp and test folders live under `AppData`.
fn in_excluded_dir(folder: &Path, base: &Path) -> bool {
    let shared = folder
        .components()
        .zip(base.components())
        .take_while(|(a, b)| {
            a.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
        })
        .count();
    folder.components().skip(shared).any(|c| match c {
        Component::Normal(name) => {
            let name = name.to_string_lossy();
            DEFAULT_EXCLUDED_DIRS
                .iter()
                .any(|ex| ex.eq_ignore_ascii_case(&name))
        }
        _ => false,
    })
}

/// True when `folder` or any ancestor up to the filesystem root contains a
/// `.valetignore` marker. Results are cached per directory for one plan, so
/// sibling destinations share the ancestor lookups.
fn under_ignore_marker(
    platform: &dyn Platform,
    folder: &Path,
    cache: &mut HashMap<PathBuf, bool>,
) -> bool {
    if let Some(&hit) = cache.get(folder) {
        return hit;
    }
    let hit = has_ignore_marker(platform, folder)
        || folder
            .parent()
            .is_some_and(|p| !p.as_os_str().is_empty() && under_ignore_marker(platform, p, cache));
    cache.insert(folder.to_path_buf(), hit);
    hit
}

/// Expands a destination folder template, in this order: leading `~`,
/// `%VAR%`, `{year}` / `{month}` (file mtime, local time), `{ext}`.
fn expand_folder(template: &str, f: &FoundFile, ctx: &PlanContext) -> Result<PathBuf, SkipReason> {
    let (home, rest) =
        if template == "~" || template.starts_with("~/") || template.starts_with("~\\") {
            let home = ctx.home_dir.clone().ok_or(SkipReason::InvalidDestination)?;
            (Some(home), template[1..].trim_start_matches(['/', '\\']))
        } else {
            (None, template)
        };

    let mut s = expand_env(rest, ctx)?;
    if s.contains("{year}") || s.contains("{month}") {
        let local: DateTime<Local> = f.mtime.into();
        s = s
            .replace("{year}", &format!("{:04}", local.year()))
            .replace("{month}", &format!("{:02}", local.month()));
    }
    if s.contains("{ext}") {
        let ext = f
            .path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .filter(|e| !e.is_empty())
            .ok_or(SkipReason::NoExtension)?;
        s = s.replace("{ext}", &ext);
    }
    Ok(match home {
        Some(h) if s.is_empty() => h,
        Some(h) => h.join(s),
        None => PathBuf::from(s),
    })
}

/// Replaces `%NAME%` with the variable's value. A lone `%` stays literal;
/// a well-formed `%NAME%` whose variable is unset is an invalid destination.
fn expand_env(s: &str, ctx: &PlanContext) -> Result<String, SkipReason> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 && !after[..end].contains(['/', '\\']) => {
                let name = &after[..end];
                let value = (ctx.env)(name).ok_or(SkipReason::InvalidDestination)?;
                out.push_str(&value);
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests;
