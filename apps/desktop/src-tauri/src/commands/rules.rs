//! Rule commands: list, upsert (validated), delete.
//!
//! # Rule JSON
//!
//! `get_rules` returns and `upsert_rule` accepts objects of this shape
//! (`valet_core::rules::Rule`):
//!
//! | field         | type                | notes |
//! |---------------|---------------------|-------|
//! | `id`          | UUID string         | new rules: the UI generates it |
//! | `name`        | string              | not blank |
//! | `enabled`     | bool                | |
//! | `alwaysApply` | bool, default false | ignores `conditions` (still files only) |
//! | `conditions`  | array of `{ "type", "value" }` | all must hold; at least one unless `alwaysApply` |
//! | `actions`     | array of `{ "type", "params" }` | at least one |
//! | `version`     | integer             | set by `upsert_rule`: +1 when anything else changed |
//! | `options`     | object, default `{}` | `includeHidden: bool`, `topLevelOnly: bool`; other keys are kept and ignored |
//! | `scope`       | array of strings, default `[]` | watched folders the rule applies to; empty = all |
//!
//! Conditions (text comparisons ignore case):
//!
//! | `type`                  | `value` |
//! |-------------------------|---------|
//! | `ext`                   | string, one extension without dot (`"pdf"`) |
//! | `extIn`, `extNotIn`     | non-empty array of extensions; a leading dot is allowed (`".pdf"`). `extNotIn` holds for files without extension |
//! | `typeGroup`             | one of `image`, `document`, `archive`, `installer`, `video`, `audio`, `code` (lists in `valet_core::typegroups`) |
//! | `nameContains`          | non-empty string, or non-empty array of non-empty strings (any of them); file name only |
//! | `nameMatches`           | string, a regular expression (Rust `regex` syntax) on the file name |
//! | `pathContains`          | non-empty string, searched in the full path |
//! | `sizeGt`, `sizeLt`      | integer >= 0, bytes |
//! | `modifiedOlderThanDays` | integer 0..=36500 |
//! | `createdOlderThanDays`  | integer 0..=36500; false where the OS gives no creation time |
//! | `emptyFolder`           | `true`; the rule then sees only empty folders (never a watched folder itself) |
//! | `duplicate`             | `true`; the file has a byte-identical copy among the files the rule's other conditions match |
//!
//! Actions:
//!
//! | `type`   | `params` |
//! |----------|----------|
//! | `moveTo` | `{ "path": string }`: destination folder, see below |
//! | `copyTo` | `{ "path": string }` |
//! | `trash`  | none (`{}` or omitted): send to the Recycle Bin; with `duplicate`, every copy except the oldest |
//! | `report` | none: list the file in the preview's `reports`, no file operation |
//!
//! Destination `path`: absolute, relative to the watched folder, or starting
//! with `~`, `%VAR%` or a folder token `{home}`, `{desktop}`, `{documents}`,
//! `{downloads}`, `{pictures}`, `{videos}`, `{music}` (as the whole first
//! component). `{year}`, `{month}`, `{day}` (file modification date) and
//! `{ext}` may appear anywhere. Other `{name}` tokens are refused.
//!
//! Scope entries: a folder token (`"{downloads}"`, optionally followed by
//! `/sub`), a path starting with `~` or `%VAR%`, or an absolute path. A rule
//! applies under a watched folder when an entry resolves to exactly that
//! folder (case-insensitive).
//!
//! Combination rules: at most one of `moveTo`/`copyTo`/`trash` per rule;
//! `emptyFolder` cannot be combined with `ext`, `extIn`, `extNotIn`,
//! `nameContains`, `nameMatches`, `typeGroup`, `sizeGt`, `sizeLt` or
//! `duplicate`, nor with `moveTo`/`copyTo`; `duplicate` cannot be combined
//! with `moveTo`/`copyTo`.
//!
//! # Errors
//!
//! Commands return a stable code as the error string; the UI translates it.
//! See the `ERR_*` constants. Storage failures return `rule.storage` (the
//! detail goes to the log).

use serde_json::Value;
use tauri::Manager;
use uuid::Uuid;
use valet_core::{
    planner::{template_tokens_valid, FOLDER_TOKENS},
    rules::{next_version, Rule},
    storage::Db,
    typegroups,
};

use crate::commands::settings::remove_approval;
use crate::state::{config_dir, AppState};

pub(crate) const ERR_NAME_EMPTY: &str = "rule.nameEmpty";
pub(crate) const ERR_NO_ACTIONS: &str = "rule.noActions";
pub(crate) const ERR_NO_CONDITIONS: &str = "rule.noConditions";
pub(crate) const ERR_UNKNOWN_CONDITION: &str = "rule.unknownCondition";
pub(crate) const ERR_CONDITION_VALUE: &str = "rule.invalidConditionValue";
pub(crate) const ERR_REGEX: &str = "rule.invalidRegex";
pub(crate) const ERR_UNKNOWN_ACTION: &str = "rule.unknownAction";
pub(crate) const ERR_ACTION_PARAMS: &str = "rule.invalidActionParams";
pub(crate) const ERR_MULTIPLE_MUTATING: &str = "rule.multipleMutatingActions";
pub(crate) const ERR_EMPTY_FOLDER_FILE_CONDITION: &str = "rule.emptyFolderFileCondition";
pub(crate) const ERR_EMPTY_FOLDER_ACTION: &str = "rule.emptyFolderAction";
pub(crate) const ERR_DUPLICATE_ACTION: &str = "rule.duplicateAction";
pub(crate) const ERR_SCOPE: &str = "rule.invalidScope";
pub(crate) const ERR_OPTIONS: &str = "rule.invalidOptions";
pub(crate) const ERR_NOT_FOUND: &str = "rule.notFound";
pub(crate) const ERR_STORAGE: &str = "rule.storage";

/// Largest accepted `*OlderThanDays` value (100 years).
const MAX_DAYS: u64 = 36_500;

/// Conditions that look at file contents or names; not allowed with `emptyFolder`.
const FILE_ONLY_CONDITIONS: &[&str] = &[
    "ext",
    "extIn",
    "extNotIn",
    "nameContains",
    "nameMatches",
    "typeGroup",
    "sizeGt",
    "sizeLt",
    "duplicate",
];

fn non_empty_str(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty())
}

/// Non-empty array whose items all satisfy `ok`.
fn non_empty_list(v: &Value, ok: impl Fn(&Value) -> bool) -> bool {
    v.as_array()
        .is_some_and(|a| !a.is_empty() && a.iter().all(ok))
}

fn ext_item(v: &Value) -> bool {
    v.as_str()
        .is_some_and(|s| !s.trim().trim_start_matches('.').is_empty())
}

fn check_condition(kind: &str, value: &Value) -> Result<(), &'static str> {
    let ok = match kind {
        "ext" | "pathContains" => non_empty_str(value),
        "extIn" | "extNotIn" => non_empty_list(value, ext_item),
        "typeGroup" => value
            .as_str()
            .is_some_and(|g| typegroups::GROUPS.contains(&g)),
        "nameContains" => non_empty_str(value) || non_empty_list(value, non_empty_str),
        "nameMatches" => {
            let pattern = value.as_str().ok_or(ERR_CONDITION_VALUE)?;
            regex::Regex::new(pattern).map_err(|_| ERR_REGEX)?;
            true
        }
        "sizeGt" | "sizeLt" => value.as_u64().is_some(),
        "modifiedOlderThanDays" | "createdOlderThanDays" => {
            value.as_u64().is_some_and(|d| d <= MAX_DAYS)
        }
        "emptyFolder" | "duplicate" => *value == Value::Bool(true),
        _ => return Err(ERR_UNKNOWN_CONDITION),
    };
    if ok {
        Ok(())
    } else {
        Err(ERR_CONDITION_VALUE)
    }
}

/// `params` is absent/null or an object.
fn no_params(params: &Value) -> bool {
    params.is_null() || params.is_object()
}

fn check_action(kind: &str, params: &Value) -> Result<(), &'static str> {
    let ok = match kind {
        "moveTo" | "copyTo" => params
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|p| !p.trim().is_empty() && template_tokens_valid(p)),
        "trash" | "report" => no_params(params),
        _ => return Err(ERR_UNKNOWN_ACTION),
    };
    if ok {
        Ok(())
    } else {
        Err(ERR_ACTION_PARAMS)
    }
}

/// `C:\x`, `C:/x`, `\\server\share`, `/x`.
fn looks_absolute(s: &str) -> bool {
    let b = s.as_bytes();
    s.starts_with('/')
        || s.starts_with("\\\\")
        || (b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && matches!(b[2], b'\\' | b'/'))
}

fn scope_entry_ok(entry: &str) -> bool {
    let e = entry.trim();
    if e.is_empty() {
        return false;
    }
    if let Some(after) = e.strip_prefix('{') {
        let Some(end) = after.find('}') else {
            return false;
        };
        let rest = &after[end + 1..];
        return FOLDER_TOKENS.contains(&&after[..end])
            && (rest.is_empty() || rest.starts_with(['/', '\\']))
            && !rest.contains(['{', '}']);
    }
    (e == "~"
        || e.starts_with("~/")
        || e.starts_with("~\\")
        || e.starts_with('%')
        || looks_absolute(e))
        && !e.contains(['{', '}'])
}

fn options_ok(options: &Value) -> bool {
    match options {
        Value::Null => true,
        Value::Object(o) => ["includeHidden", "topLevelOnly"]
            .iter()
            .all(|k| o.get(*k).map_or(true, Value::is_boolean)),
        _ => false,
    }
}

fn is_mutating(kind: &str) -> bool {
    matches!(kind, "moveTo" | "copyTo" | "trash")
}

fn is_transfer(kind: &str) -> bool {
    matches!(kind, "moveTo" | "copyTo")
}

/// Accepts exactly the condition and action types the planner implements,
/// with typed values. Returns the first problem as an error code.
pub(crate) fn validate_rule(rule: &Rule) -> Result<(), &'static str> {
    if rule.name.trim().is_empty() {
        return Err(ERR_NAME_EMPTY);
    }
    if rule.actions.is_empty() {
        return Err(ERR_NO_ACTIONS);
    }
    if rule.conditions.is_empty() && !rule.always_apply {
        return Err(ERR_NO_CONDITIONS);
    }
    for c in &rule.conditions {
        check_condition(&c.r#type, &c.value)?;
    }
    for a in &rule.actions {
        check_action(&a.r#type, &a.params)?;
    }
    if rule
        .actions
        .iter()
        .filter(|a| is_mutating(&a.r#type))
        .count()
        > 1
    {
        return Err(ERR_MULTIPLE_MUTATING);
    }
    let has_cond = |t: &str| rule.conditions.iter().any(|c| c.r#type == t);
    let has_transfer = rule.actions.iter().any(|a| is_transfer(&a.r#type));
    if has_cond("emptyFolder") {
        if rule
            .conditions
            .iter()
            .any(|c| FILE_ONLY_CONDITIONS.contains(&c.r#type.as_str()))
        {
            return Err(ERR_EMPTY_FOLDER_FILE_CONDITION);
        }
        if has_transfer {
            return Err(ERR_EMPTY_FOLDER_ACTION);
        }
    }
    if has_cond("duplicate") && has_transfer {
        return Err(ERR_DUPLICATE_ACTION);
    }
    if !rule.scope.iter().all(|e| scope_entry_ok(e)) {
        return Err(ERR_SCOPE);
    }
    if !options_ok(&rule.options) {
        return Err(ERR_OPTIONS);
    }
    Ok(())
}

fn storage_err(e: impl std::fmt::Display) -> String {
    log::warn!("Rule storage failed: {e}");
    ERR_STORAGE.to_string()
}

pub(crate) async fn connect(app: &tauri::AppHandle) -> Result<Db, String> {
    let state = app.state::<AppState>();
    Db::connect(&state._db_path).await.map_err(storage_err)
}

#[tauri::command]
pub(crate) async fn get_rules(app: tauri::AppHandle) -> Result<Vec<Rule>, String> {
    connect(&app).await?.list_rules().await.map_err(storage_err)
}

/// Validates and stores `rule`; returns it as stored (with its new `version`).
pub(crate) async fn save_rule(db: &Db, mut rule: Rule) -> Result<Rule, String> {
    validate_rule(&rule).map_err(String::from)?;
    let stored = db.get_rule(rule.id).await.map_err(storage_err)?;
    rule.version = next_version(stored.as_ref(), &rule);
    db.upsert_rule(&rule).await.map_err(storage_err)?;
    Ok(rule)
}

#[tauri::command]
pub(crate) async fn upsert_rule(rule: Rule, app: tauri::AppHandle) -> Result<Rule, String> {
    let db = connect(&app).await?;
    let saved = save_rule(&db, rule).await?;
    log::info!("Rule saved (version {})", saved.version);
    Ok(saved)
}

/// Removes the rule row. No file is touched.
pub(crate) async fn remove_rule(db: &Db, rule_id: Uuid) -> Result<(), String> {
    if db.delete_rule(rule_id).await.map_err(storage_err)? {
        Ok(())
    } else {
        Err(ERR_NOT_FOUND.to_string())
    }
}

/// Removes the rule row and its approval in `approvals.json` (config folder
/// `dir`), so a rule created later with the same id needs a new preview.
pub(crate) async fn remove_rule_and_approval(
    db: &Db,
    dir: &std::path::Path,
    rule_id: Uuid,
) -> Result<(), String> {
    remove_rule(db, rule_id).await?;
    remove_approval(dir, rule_id)
}

/// Deletes the rule with `ruleId` (JS argument name) and its approval.
/// Error `rule.notFound` when there is none.
#[tauri::command]
pub(crate) async fn delete_rule(rule_id: Uuid, app: tauri::AppHandle) -> Result<(), String> {
    let db = connect(&app).await?;
    remove_rule_and_approval(&db, &config_dir(&app)?, rule_id).await?;
    log::info!("Rule deleted");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rule(conditions: Value, actions: Value) -> Rule {
        serde_json::from_value(json!({
            "id": Uuid::from_u128(1),
            "name": "r",
            "enabled": true,
            "conditions": conditions,
            "actions": actions,
        }))
        .unwrap()
    }

    fn cond(t: &str, v: Value) -> Rule {
        rule(
            json!([{ "type": t, "value": v }]),
            json!([{ "type": "report" }]),
        )
    }

    fn action(t: &str, params: Value) -> Rule {
        rule(
            json!([{ "type": "ext", "value": "pdf" }]),
            json!([{ "type": t, "params": params }]),
        )
    }

    #[test]
    fn accepts_every_implemented_condition_with_a_typed_value() {
        for (t, v) in [
            ("ext", json!("pdf")),
            ("extIn", json!(["pdf", ".docx"])),
            ("extNotIn", json!(["exe"])),
            ("typeGroup", json!("installer")),
            ("nameContains", json!("factura")),
            ("nameContains", json!(["invoice", "bon"])),
            ("nameMatches", json!("(?i)^scan_\\d+")),
            ("pathContains", json!("work")),
            ("sizeGt", json!(1_073_741_824u64)),
            ("sizeLt", json!(0)),
            ("modifiedOlderThanDays", json!(30)),
            ("createdOlderThanDays", json!(0)),
            ("duplicate", json!(true)),
        ] {
            assert_eq!(validate_rule(&cond(t, v.clone())), Ok(()), "{t} {v}");
        }
        assert_eq!(validate_rule(&cond("emptyFolder", json!(true))), Ok(()));
    }

    #[test]
    fn rejects_malformed_condition_values() {
        for (t, v) in [
            ("ext", json!("")),
            ("ext", json!(1)),
            ("extIn", json!([])),
            ("extIn", json!("pdf")),
            ("extNotIn", json!(["."])),
            ("extIn", json!(["pdf", 2])),
            ("typeGroup", json!("Installer")),
            ("typeGroup", json!("other")),
            ("nameContains", json!("")),
            ("nameContains", json!([])),
            ("nameContains", json!(["a", ""])),
            ("nameMatches", json!(5)),
            ("pathContains", json!("  ")),
            ("sizeGt", json!(-1)),
            ("sizeLt", json!("10")),
            ("modifiedOlderThanDays", json!(1.5)),
            ("createdOlderThanDays", json!(-3)),
            ("modifiedOlderThanDays", json!(36_501)),
            ("emptyFolder", json!(false)),
            ("duplicate", json!("true")),
        ] {
            assert_eq!(
                validate_rule(&cond(t, v.clone())),
                Err(ERR_CONDITION_VALUE),
                "{t} {v}"
            );
        }
    }

    #[test]
    fn rejects_bad_regex_and_unknown_condition() {
        assert_eq!(
            validate_rule(&cond("nameMatches", json!("[unclosed"))),
            Err(ERR_REGEX)
        );
        assert_eq!(
            validate_rule(&cond("mimeIs", json!("image/png"))),
            Err(ERR_UNKNOWN_CONDITION)
        );
    }

    #[test]
    fn accepts_implemented_actions_and_rejects_others() {
        for (t, p) in [
            ("moveTo", json!({ "path": "{downloads}/Imagini/{year}" })),
            ("copyTo", json!({ "path": "C:\\Backup" })),
            ("trash", json!(null)),
            ("trash", json!({})),
            ("report", json!(null)),
        ] {
            assert_eq!(validate_rule(&action(t, p.clone())), Ok(()), "{t} {p}");
        }
        for t in ["tag", "rename", "quarantine", "delete"] {
            assert_eq!(
                validate_rule(&action(t, json!({}))),
                Err(ERR_UNKNOWN_ACTION),
                "{t}"
            );
        }
    }

    #[test]
    fn rejects_bad_action_params() {
        for (t, p) in [
            ("moveTo", json!({})),
            ("moveTo", json!({ "path": "  " })),
            ("copyTo", json!({ "path": 3 })),
            ("moveTo", json!({ "path": "Out/{nope}" })),
            ("moveTo", json!({ "path": "Out/{desktop}" })),
            ("trash", json!("now")),
            ("report", json!([1])),
        ] {
            assert_eq!(
                validate_rule(&action(t, p.clone())),
                Err(ERR_ACTION_PARAMS),
                "{t} {p}"
            );
        }
    }

    #[test]
    fn requires_name_actions_and_conditions_unless_always_apply() {
        let mut r = cond("ext", json!("pdf"));
        r.name = "  ".into();
        assert_eq!(validate_rule(&r), Err(ERR_NAME_EMPTY));

        let r = rule(json!([{ "type": "ext", "value": "pdf" }]), json!([]));
        assert_eq!(validate_rule(&r), Err(ERR_NO_ACTIONS));

        let mut r = rule(json!([]), json!([{ "type": "report" }]));
        assert_eq!(validate_rule(&r), Err(ERR_NO_CONDITIONS));
        r.always_apply = true;
        assert_eq!(validate_rule(&r), Ok(()));
    }

    #[test]
    fn rejects_more_than_one_mutating_action() {
        let path = json!({ "path": "Out" });
        for actions in [
            json!([{ "type": "moveTo", "params": path }, { "type": "trash" }]),
            json!([{ "type": "moveTo", "params": path }, { "type": "copyTo", "params": path }]),
            json!([{ "type": "trash" }, { "type": "trash" }]),
        ] {
            let r = rule(json!([{ "type": "ext", "value": "pdf" }]), actions.clone());
            assert_eq!(validate_rule(&r), Err(ERR_MULTIPLE_MUTATING), "{actions}");
        }
        let r = rule(
            json!([{ "type": "ext", "value": "exe" }]),
            json!([{ "type": "report" }, { "type": "trash" }]),
        );
        assert_eq!(validate_rule(&r), Ok(()), "report is not mutating");
    }

    #[test]
    fn empty_folder_rejects_file_conditions_and_transfers() {
        for (t, v) in [
            ("ext", json!("pdf")),
            ("extIn", json!(["pdf"])),
            ("extNotIn", json!(["pdf"])),
            ("nameContains", json!("x")),
            ("nameMatches", json!("x")),
            ("typeGroup", json!("image")),
            ("sizeGt", json!(1)),
            ("sizeLt", json!(1)),
            ("duplicate", json!(true)),
        ] {
            let r = rule(
                json!([{ "type": "emptyFolder", "value": true }, { "type": t, "value": v }]),
                json!([{ "type": "trash" }]),
            );
            assert_eq!(
                validate_rule(&r),
                Err(ERR_EMPTY_FOLDER_FILE_CONDITION),
                "{t}"
            );
        }
        let r = rule(
            json!([
                { "type": "emptyFolder", "value": true },
                { "type": "modifiedOlderThanDays", "value": 7 },
                { "type": "pathContains", "value": "x" },
            ]),
            json!([{ "type": "trash" }]),
        );
        assert_eq!(validate_rule(&r), Ok(()), "age and path are allowed");
        let r = rule(
            json!([{ "type": "emptyFolder", "value": true }]),
            json!([{ "type": "moveTo", "params": { "path": "Out" } }]),
        );
        assert_eq!(validate_rule(&r), Err(ERR_EMPTY_FOLDER_ACTION));
    }

    #[test]
    fn duplicate_allows_report_and_trash_only() {
        let r = rule(
            json!([{ "type": "duplicate", "value": true }]),
            json!([{ "type": "copyTo", "params": { "path": "Out" } }]),
        );
        assert_eq!(validate_rule(&r), Err(ERR_DUPLICATE_ACTION));
        let r = rule(
            json!([{ "type": "duplicate", "value": true }]),
            json!([{ "type": "report" }, { "type": "trash" }]),
        );
        assert_eq!(validate_rule(&r), Ok(()));
    }

    #[test]
    fn scope_entries_must_be_tokens_or_absolute() {
        for ok in [
            "{downloads}",
            "{desktop}\\Proiecte",
            "~/Downloads",
            "%USERPROFILE%\\Downloads",
            "C:\\Users\\Ana\\Downloads",
            "D:/Media",
            "\\\\nas\\share\\in",
            "/home/ana/in",
        ] {
            let mut r = cond("ext", json!("pdf"));
            r.scope = vec![ok.into()];
            assert_eq!(validate_rule(&r), Ok(()), "{ok}");
        }
        for bad in [
            "",
            "Downloads",
            "{nope}",
            "{downloads}x",
            "{downloads}/{year}",
            "C:x",
        ] {
            let mut r = cond("ext", json!("pdf"));
            r.scope = vec!["{downloads}".into(), bad.into()];
            assert_eq!(validate_rule(&r), Err(ERR_SCOPE), "{bad:?}");
        }
    }

    #[test]
    fn options_flags_must_be_booleans() {
        for ok in [
            json!(null),
            json!({}),
            json!({ "topLevelOnly": true, "askConfirmation": 1 }),
        ] {
            let mut r = cond("ext", json!("pdf"));
            r.options = ok.clone();
            assert_eq!(validate_rule(&r), Ok(()), "{ok}");
        }
        for bad in [
            json!({ "includeHidden": "yes" }),
            json!({ "topLevelOnly": 1 }),
            json!([]),
        ] {
            let mut r = cond("ext", json!("pdf"));
            r.options = bad.clone();
            assert_eq!(validate_rule(&r), Err(ERR_OPTIONS), "{bad}");
        }
    }

    async fn db() -> (tempfile::TempDir, Db) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("valet.sqlite3"))
            .await
            .unwrap();
        (tmp, db)
    }

    #[tokio::test]
    async fn save_rule_bumps_version_only_when_content_changes() {
        let (_tmp, db) = db().await;
        let r = cond("ext", json!("pdf"));
        assert_eq!(save_rule(&db, r.clone()).await.unwrap().version, 1);
        assert_eq!(save_rule(&db, r.clone()).await.unwrap().version, 1);
        let mut changed = r.clone();
        changed.name = "other".into();
        assert_eq!(save_rule(&db, changed.clone()).await.unwrap().version, 2);
        // The UI sending a stale version does not reset it.
        assert_eq!(save_rule(&db, changed).await.unwrap().version, 2);
        assert_eq!(db.get_rule(r.id).await.unwrap().unwrap().version, 2);
    }

    #[tokio::test]
    async fn save_rule_refuses_invalid_rule_and_stores_nothing() {
        let (_tmp, db) = db().await;
        let r = rule(json!([]), json!([{ "type": "trash" }]));
        assert_eq!(save_rule(&db, r).await.unwrap_err(), ERR_NO_CONDITIONS);
        assert!(db.list_rules().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn remove_rule_deletes_row_or_reports_not_found() {
        let (_tmp, db) = db().await;
        let r = cond("ext", json!("pdf"));
        save_rule(&db, r.clone()).await.unwrap();
        remove_rule(&db, r.id).await.unwrap();
        assert!(db.list_rules().await.unwrap().is_empty());
        assert_eq!(remove_rule(&db, r.id).await.unwrap_err(), ERR_NOT_FOUND);
    }

    #[tokio::test]
    async fn deleting_a_rule_removes_its_approval() {
        use crate::commands::settings::{
            load_approvals, record_preview_approval, root_key, PreviewApproval,
        };
        let (tmp, db) = db().await;
        let dir = tmp.path().join("cfg");
        let r = cond("ext", json!("pdf"));
        let mut other = cond("ext", json!("txt"));
        other.id = Uuid::from_u128(2);
        save_rule(&db, r.clone()).await.unwrap();
        save_rule(&db, other.clone()).await.unwrap();
        let inbox = std::path::PathBuf::from("C:\\in");
        let approval = PreviewApproval {
            rules: vec![(r.id, 1), (other.id, 1)],
            roots: vec![inbox.clone()],
        };
        record_preview_approval(&dir, &approval).unwrap();

        remove_rule_and_approval(&db, &dir, r.id).await.unwrap();
        let a = load_approvals(&dir);
        let k = root_key(&inbox);
        assert!(!a.allows(&r, &k), "a new rule with this id needs a preview");
        assert!(a.allows(&other, &k));
        assert_eq!(
            remove_rule_and_approval(&db, &dir, r.id).await.unwrap_err(),
            ERR_NOT_FOUND
        );
    }
}
