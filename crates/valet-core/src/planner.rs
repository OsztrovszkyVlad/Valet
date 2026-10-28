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

/// Folder tokens (names without braces) usable at the start of a destination
/// template and as rule scope entries: `{home}`, `{desktop}`, ...
pub const FOLDER_TOKENS: &[&str] = &[
    "home",
    "desktop",
    "documents",
    "downloads",
    "pictures",
    "videos",
    "music",
];

/// Inputs from the environment, injected so plans are deterministic in tests.
pub struct PlanContext {
    pub now: SystemTime,
    pub home_dir: Option<PathBuf>,
    pub env: EnvLookup,
    /// Known folders by token name without braces (`"downloads"`). A token
    /// missing here makes the destination invalid; `{home}` falls back to
    /// `home_dir`.
    pub folders: HashMap<String, PathBuf>,
}

impl PlanContext {
    /// Current time, the user's home directory, the process environment and
    /// the user's known folders (`directories::UserDirs`; Windows known folders).
    pub fn from_system() -> Self {
        let mut folders = HashMap::new();
        if let Some(u) = directories::UserDirs::new() {
            folders.insert("home".to_owned(), u.home_dir().to_path_buf());
            for (token, dir) in [
                ("desktop", u.desktop_dir()),
                ("documents", u.document_dir()),
                ("downloads", u.download_dir()),
                ("pictures", u.picture_dir()),
                ("videos", u.video_dir()),
                ("music", u.audio_dir()),
            ] {
                if let Some(dir) = dir {
                    folders.insert(token.to_owned(), dir.to_path_buf());
                }
            }
        }
        Self {
            now: SystemTime::now(),
            home_dir: directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf()),
            env: Box::new(|key| std::env::var(key).ok()),
            folders,
        }
    }

    /// Path for folder token `name` (without braces).
    fn folder(&self, name: &str) -> Option<PathBuf> {
        self.folders
            .get(name)
            .cloned()
            .or_else(|| (name == "home").then(|| self.home_dir.clone()).flatten())
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
    let prepared = prepare(roots, rules, ctx);
    scan(roots, platform, ctx.now, |found| match found {
        Found::Skip(s) => out.skipped.push(s),
        Found::File(f) => plan_file(
            &f,
            &prepared,
            platform,
            ctx,
            &mut reserved,
            &mut marker_cache,
            &mut out,
        ),
    });
    out
}

// ---------------------------------------------------------------- rule scope

/// A rule with its scope resolved against the roots of one plan.
struct Prepared<'r> {
    rule: &'r Rule,
    /// `in_roots[i]`: the rule applies under `roots[i]`.
    in_roots: Vec<bool>,
    /// `options.topLevelOnly`: only entries directly inside a root.
    top_level_only: bool,
}

impl Prepared<'_> {
    fn sees(&self, f: &FoundFile) -> bool {
        self.in_roots.get(f.root_idx).copied().unwrap_or(false)
            && (!self.top_level_only || f.depth <= 1)
    }
}

fn prepare<'r>(roots: &[PathBuf], rules: &'r [Rule], ctx: &PlanContext) -> Vec<Prepared<'r>> {
    let root_keys: Vec<String> = roots.iter().map(|r| fold(r)).collect();
    rules
        .iter()
        .map(|rule| {
            let in_roots = if rule.scope.is_empty() {
                vec![true; roots.len()]
            } else {
                let scope: HashSet<String> = rule
                    .scope
                    .iter()
                    .filter_map(|e| expand_scope_entry(e, ctx))
                    .map(|p| fold(&p))
                    .collect();
                root_keys.iter().map(|k| scope.contains(k)).collect()
            };
            Prepared {
                rule,
                in_roots,
                top_level_only: rule.option_flag("topLevelOnly"),
            }
        })
        .collect()
}

/// Absolute folder for a scope entry: a folder token, `~`, `%VAR%` or an
/// absolute path. `None` (the entry matches no root) when it cannot be
/// resolved, contains another `{token}`, or is relative.
fn expand_scope_entry(entry: &str, ctx: &PlanContext) -> Option<PathBuf> {
    let (base, rest) = expand_prefix(entry.trim(), ctx).ok()?;
    let rest = expand_env(rest, ctx).ok()?;
    if find_token(&rest).is_some() {
        return None;
    }
    let p = match base {
        Some(b) if rest.is_empty() => b,
        Some(b) => b.join(rest),
        None => PathBuf::from(rest),
    };
    p.is_absolute().then_some(p)
}

// ---------------------------------------------------------------- walker

/// A regular file that passed the walker's checks.
pub(crate) struct FoundFile {
    pub path: PathBuf,
    /// Index of the scan root in the `roots` given to `scan`.
    pub root_idx: usize,
    /// Depth below the root (1 = directly inside; 0 = the root is this file).
    pub depth: usize,
    /// Folder that relative destinations resolve against (the scan root,
    /// or its parent when the root is a file).
    pub base: PathBuf,
    pub size: u64,
    pub mtime: SystemTime,
    pub created: Option<SystemTime>,
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
    for (root_idx, root) in roots.iter().enumerate() {
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
                root_idx,
                depth: entry.depth(),
                base: base.clone(),
                size: meta.len(),
                mtime,
                created: meta.created().ok(),
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
    rule.option_flag("includeHidden")
}

fn plan_file(
    f: &FoundFile,
    rules: &[Prepared],
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
        mtime: Some(f.mtime),
        created: f.created,
        is_dir: false,
        is_empty_dir: false,
        now: Some(ctx.now),
    };
    let matching: Vec<&Rule> = rules
        .iter()
        .filter(|p| p.sees(f) && rule_matches(p.rule, &facts))
        .map(|p| p.rule)
        .collect();
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

/// Splits a leading `~` or folder token (`{downloads}`, see
/// [`FOLDER_TOKENS`]) off `template`. A folder token counts only as the whole
/// first component. Unset home or an unknown folder: invalid destination.
fn expand_prefix<'t>(
    template: &'t str,
    ctx: &PlanContext,
) -> Result<(Option<PathBuf>, &'t str), SkipReason> {
    let sep = |r: &str| r.is_empty() || r.starts_with(['/', '\\']);
    if let Some(rest) = template.strip_prefix('~')
        && sep(rest)
    {
        let home = ctx.home_dir.clone().ok_or(SkipReason::InvalidDestination)?;
        return Ok((Some(home), rest.trim_start_matches(['/', '\\'])));
    }
    if let Some((start, end)) = find_token(template)
        && start == 0
        && FOLDER_TOKENS.contains(&&template[1..end - 1])
        && sep(&template[end..])
    {
        let dir = ctx
            .folder(&template[1..end - 1])
            .ok_or(SkipReason::InvalidDestination)?;
        return Ok((Some(dir), template[end..].trim_start_matches(['/', '\\'])));
    }
    Ok((None, template))
}

/// Byte range `start..end` of the first `{name}` in `s` whose name is one or
/// more ASCII letters or `_`. Other braces are literal text.
fn find_token(s: &str) -> Option<(usize, usize)> {
    let mut from = 0;
    while let Some(i) = s[from..].find('{') {
        let start = from + i;
        let after = &s[start + 1..];
        let len = after
            .bytes()
            .take_while(|b| b.is_ascii_alphabetic() || *b == b'_')
            .count();
        if len > 0 && after.as_bytes().get(len) == Some(&b'}') {
            return Some((start, start + len + 2));
        }
        from = start + 1;
    }
    None
}

/// Expands a destination folder template, in this order: a leading `~` or
/// folder token, `%VAR%`, then `{year}` / `{month}` / `{day}` (file mtime,
/// local time) and `{ext}`. Token names are lowercase; any other `{name}`
/// (including a folder token after the first component) is an invalid
/// destination.
fn expand_folder(template: &str, f: &FoundFile, ctx: &PlanContext) -> Result<PathBuf, SkipReason> {
    let (base, rest) = expand_prefix(template, ctx)?;
    let s = expand_env(rest, ctx)?;
    let s = expand_file_tokens(&s, f)?;
    Ok(match base {
        Some(b) if s.is_empty() => b,
        Some(b) => b.join(s),
        None => PathBuf::from(s),
    })
}

fn expand_file_tokens(s: &str, f: &FoundFile) -> Result<String, SkipReason> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    let mut local: Option<DateTime<Local>> = None;
    while let Some((start, end)) = find_token(rest) {
        out.push_str(&rest[..start]);
        let mut date = || *local.get_or_insert_with(|| f.mtime.into());
        match &rest[start + 1..end - 1] {
            "year" => out.push_str(&format!("{:04}", date().year())),
            "month" => out.push_str(&format!("{:02}", date().month())),
            "day" => out.push_str(&format!("{:02}", date().day())),
            "ext" => {
                let ext = f
                    .path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .filter(|e| !e.is_empty())
                    .ok_or(SkipReason::NoExtension)?;
                out.push_str(&ext);
            }
            _ => return Err(SkipReason::InvalidDestination),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    Ok(out)
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
