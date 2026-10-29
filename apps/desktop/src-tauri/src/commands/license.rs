//! License and plan commands (`docs/LICENSING.md`, "App commands").
//!
//! | command | JS args | returns |
//! |---|---|---|
//! | `get_plan_status` | — | `PlanView` |
//! | `import_license_file` | — | `PlanView \| null` (null: dialog cancelled) |
//! | `import_license_text` | `text` | `PlanView` |
//! | `check_license_now` | — | `PlanView` |
//! | `remove_license` | — | `PlanView` |
//! | `apply_plan_choice` | `ruleIds: string[]`, `folders: string[]` | `PlanView` |
//! | `export_rules_file` | — | `string \| null`: the written path (null: cancelled) |
//! | `import_rules_file` | — | `Rule[] \| null`: the stored rules (null: cancelled) |
//!
//! Dialogs are opened from Rust on a blocking thread; the webview has no
//! dialog permission.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;
use valet_core::entitlements::{LimitKind, PlanLimit, FREE_FOLDERS, FREE_OWN_RULES, PLAN_DISABLED};
use valet_core::license::{PlanTier, Reason};
use valet_core::rule_transfer::{export_rules, import_rules, ImportError, MAX_IMPORT_BYTES};
use valet_core::rules::Rule;
use valet_core::storage::Db;

use crate::commands::rules::{validate_rule, ERR_NOT_FOUND};
use crate::commands::settings::{read_config, write_config, Config, ERR_MISSING};
use crate::commands::CmdError;
use crate::i18n;
use crate::licensing::{
    after_change, connect, import_in, license_dir, remove_in, sync_plan, unix_now, validate_in,
    PlanView, ERR_IO,
};
use crate::state::{config_dir, AppState};

pub(crate) const ERR_NOT_FREE: &str = "plan.notFree";
pub(crate) const ERR_PRO_ONLY: &str = "plan.proOnly";
pub(crate) const ERR_EXPORT_EXISTS: &str = "export.exists";
pub(crate) const ERR_EXPORT_IO: &str = "export.io";
pub(crate) const ERR_IMPORT_IO: &str = "import.io";

/// License files larger than this are refused like a bad signature.
const MAX_LICENSE_BYTES: u64 = 64 * 1024;

/// The plan after a change, synced (transitions applied).
async fn view_now<R: Runtime>(app: &AppHandle<R>, db: &Db) -> Result<PlanView, String> {
    Ok(sync_plan(app, db).await?.view)
}

#[tauri::command]
pub(crate) async fn get_plan_status(app: AppHandle) -> Result<PlanView, String> {
    let db = connect(&app).await?;
    view_now(&app, &db).await
}

#[tauri::command]
pub(crate) async fn import_license_text(text: String, app: AppHandle) -> Result<PlanView, String> {
    let state = app.state::<AppState>();
    let dir = license_dir(&app)?;
    import_in(&state.license, &dir, &text, unix_now()).await?;
    let db = connect(&app).await?;
    view_now(&app, &db).await
}

/// Which file a dialog is for.
#[derive(Clone, Copy)]
enum FileKind {
    License,
    Rules,
}

fn filter(app: &AppHandle, kind: FileKind) -> (&'static str, &'static [&'static str]) {
    let text = i18n::text(i18n::current_lang(app));
    match kind {
        FileKind::License => (text.license_file_filter, &["valet-license"]),
        FileKind::Rules => (text.rules_file_filter, &["json"]),
    }
}

/// Native open dialog on a blocking thread. `None` when cancelled.
async fn pick_file(app: &AppHandle, kind: FileKind) -> Result<Option<PathBuf>, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (name, ext) = filter(&app, kind);
        app.dialog()
            .file()
            .add_filter(name, ext)
            .blocking_pick_file()
            .and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| format!("dialog: {e}"))
}

/// Native save dialog on a blocking thread. `None` when cancelled.
async fn pick_save_path(
    app: &AppHandle,
    kind: FileKind,
    file_name: &str,
) -> Result<Option<PathBuf>, String> {
    let app = app.clone();
    let file_name = file_name.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let (name, ext) = filter(&app, kind);
        app.dialog()
            .file()
            .add_filter(name, ext)
            .set_file_name(file_name)
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| format!("dialog: {e}"))
}

/// The text of a picked license file. Larger than 64 KiB: as a bad
/// signature (it cannot be a license).
fn read_license_file(path: &Path) -> Result<String, String> {
    let len = std::fs::metadata(path)
        .map_err(|e| io_code(ERR_IO, e))?
        .len();
    if len > MAX_LICENSE_BYTES {
        return Err(Reason::LicenseInvalidSignature.code().into());
    }
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            Err(Reason::LicenseInvalidSignature.code().into())
        }
        Err(e) => Err(io_code(ERR_IO, e)),
    }
}

fn io_code(code: &str, e: impl std::fmt::Display) -> String {
    log::warn!("{code}: {e}");
    code.to_string()
}

#[tauri::command]
pub(crate) async fn import_license_file(app: AppHandle) -> Result<Option<PlanView>, String> {
    let Some(path) = pick_file(&app, FileKind::License).await? else {
        return Ok(None);
    };
    let text = read_license_file(&path)?;
    import_license_text(text, app).await.map(Some)
}

/// Validates now, even before 30 days. The plan is synced also after a
/// refusal (404, 409, 410 remove the token), then the error is returned.
#[tauri::command]
pub(crate) async fn check_license_now(app: AppHandle) -> Result<PlanView, String> {
    let state = app.state::<AppState>();
    let dir = license_dir(&app)?;
    let result = validate_in(&state.license, &dir, unix_now()).await;
    let db = connect(&app).await?;
    let view = view_now(&app, &db).await?;
    result.map(|()| view)
}

#[tauri::command]
pub(crate) async fn remove_license(app: AppHandle) -> Result<PlanView, String> {
    let state = app.state::<AppState>();
    let dir = license_dir(&app)?;
    remove_in(&state.license, &dir, unix_now()).await?;
    let db = connect(&app).await?;
    view_now(&app, &db).await
}

// ---------------------------------------------------------------- plan choice

/// `apply_plan_choice` on Free: the own rules in `rule_ids` (at most 3)
/// are enabled (also ones the user had disabled: picking is the user's
/// action), the other enabled own rules are disabled with `planDisabled`;
/// plan-disabled and user-disabled rules that were not picked stay as they
/// are. The `folders` (at most 2, each in `inbox_paths`) move to the front
/// of `inbox_paths` in the given order. Clears `plan_choice_pending`. Rules
/// keep their `version`. Ids of preset rules are ignored (presets are not
/// limited). Unknown ids: `rule.notFound`; unknown folders:
/// `config.missing: <path>`. Nothing is written before every check passed.
pub(crate) async fn apply_choice_in(
    db: &Db,
    dir: &Path,
    plan: &PlanTier,
    rule_ids: &[Uuid],
    folders: &[String],
) -> Result<(), CmdError> {
    if !plan.is_free() {
        return Err(ERR_NOT_FREE.into());
    }
    let rules = db.list_rules().await.map_err(storage)?;
    let mut chosen: HashSet<Uuid> = HashSet::new();
    for id in rule_ids {
        let rule = rules
            .iter()
            .find(|r| r.id == *id)
            .ok_or_else(|| CmdError::from(ERR_NOT_FOUND))?;
        if !rule.is_preset() {
            chosen.insert(*id);
        }
    }
    if chosen.len() > FREE_OWN_RULES {
        return Err(PlanLimit {
            kind: LimitKind::Rules,
            limit: FREE_OWN_RULES,
        }
        .into());
    }
    let mut picked: Vec<String> = Vec::new();
    for f in folders {
        if !picked.contains(f) {
            picked.push(f.clone());
        }
    }
    if picked.len() > FREE_FOLDERS {
        return Err(PlanLimit {
            kind: LimitKind::Folders,
            limit: FREE_FOLDERS,
        }
        .into());
    }
    let config = read_config(dir)?;
    if let Some(missing) = picked.iter().find(|f| !config.inbox_paths.contains(f)) {
        return Err(format!("{ERR_MISSING}: {missing}").into());
    }

    for rule in rules.iter().filter(|r| !r.is_preset()) {
        let mut r = rule.clone();
        let changed = if chosen.contains(&r.id) {
            let had_flag = r
                .options
                .as_object_mut()
                .and_then(|o| o.remove(PLAN_DISABLED))
                .is_some();
            let was_enabled = std::mem::replace(&mut r.enabled, true);
            had_flag || !was_enabled
        } else if r.enabled {
            r.enabled = false;
            if !r.options.is_object() {
                r.options = serde_json::json!({});
            }
            if let Some(o) = r.options.as_object_mut() {
                o.insert(PLAN_DISABLED.into(), serde_json::Value::Bool(true));
            }
            true
        } else {
            false
        };
        if changed {
            db.upsert_rule(&r).await.map_err(storage)?;
        }
    }

    let mut inbox_paths = picked.clone();
    inbox_paths.extend(
        config
            .inbox_paths
            .iter()
            .filter(|p| !picked.contains(p))
            .cloned(),
    );
    if inbox_paths != config.inbox_paths {
        write_config(dir, &Config { inbox_paths })?;
    }
    db.set_plan_choice_pending(false).await.map_err(storage)?;
    log::info!(
        "Plan choice applied: {} rule(s), {} folder(s)",
        chosen.len(),
        picked.len()
    );
    Ok(())
}

fn storage(e: impl std::fmt::Display) -> String {
    log::warn!("Plan choice storage failed: {e:#}");
    "rule.storage".to_string()
}

#[tauri::command]
pub(crate) async fn apply_plan_choice(
    rule_ids: Vec<Uuid>,
    folders: Vec<String>,
    app: AppHandle,
) -> Result<PlanView, CmdError> {
    let db = connect(&app).await?;
    let plan = sync_plan(&app, &db).await?.status.plan;
    let state = app.state::<AppState>();
    let dir = config_dir(&app)?;
    {
        let _rules = state.rule_writes.lock().await;
        apply_choice_in(&db, &dir, &plan, &rule_ids, &folders).await?;
    }
    let view = view_now(&app, &db).await?;
    // The active folders and rules changed.
    after_change(&app, &state, &dir, &view);
    Ok(view)
}

// ---------------------------------------------------------------- export / import

/// Writes the export of `rules` to `path`, refusing an existing file
/// (`export.exists`; the file is created with create-new, never replaced).
pub(crate) fn export_to(path: &Path, rules: &[Rule]) -> Result<(), String> {
    use std::io::Write;
    let text = export_rules(rules);
    let mut f = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(ERR_EXPORT_EXISTS.into())
        }
        Err(e) => return Err(io_code(ERR_EXPORT_IO, e)),
    };
    f.write_all(text.as_bytes())
        .and_then(|()| f.sync_all())
        .map_err(|e| io_code(ERR_EXPORT_IO, e))
}

/// Stores the rules of an export file as new own rules (fresh ids,
/// `version = 1`, so no approval matches: auto-apply waits for a preview).
pub(crate) async fn import_from(db: &Db, path: &Path) -> Result<Vec<Rule>, String> {
    let len = std::fs::metadata(path)
        .map_err(|e| io_code(ERR_IMPORT_IO, e))?
        .len();
    if len > MAX_IMPORT_BYTES as u64 {
        return Err(ImportError::TooLarge.code().into());
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
            return Err(ImportError::Malformed.code().into())
        }
        Err(e) => return Err(io_code(ERR_IMPORT_IO, e)),
    };
    let rules = import_rules(&text, validate_rule).map_err(|e| e.to_string())?;
    for r in &rules {
        db.upsert_rule(r).await.map_err(storage)?;
    }
    Ok(rules)
}

fn require_export_import(plan: &PlanTier) -> Result<(), String> {
    if plan.is_free() {
        Err(ERR_PRO_ONLY.into())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub(crate) async fn export_rules_file(app: AppHandle) -> Result<Option<String>, String> {
    let db = connect(&app).await?;
    require_export_import(&sync_plan(&app, &db).await?.status.plan)?;
    let Some(path) = pick_save_path(&app, FileKind::Rules, "valet-rules.json").await? else {
        return Ok(None);
    };
    let rules = db.list_rules().await.map_err(storage)?;
    export_to(&path, &rules)?;
    log::info!("Rules exported");
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub(crate) async fn import_rules_file(app: AppHandle) -> Result<Option<Vec<Rule>>, String> {
    let db = connect(&app).await?;
    require_export_import(&sync_plan(&app, &db).await?.status.plan)?;
    let Some(path) = pick_file(&app, FileKind::Rules).await? else {
        return Ok(None);
    };
    let state = app.state::<AppState>();
    let _rules = state.rule_writes.lock().await;
    let rules = import_from(&db, &path).await?;
    log::info!("Rules imported: {}", rules.len());
    Ok(Some(rules))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::licensing::tests::{db, own_rule, preset_rule};
    use serde_json::json;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    async fn setup(dir: &Path) -> Db {
        let db = db(dir).await;
        for n in 1..=5 {
            db.upsert_rule(&own_rule(n, true)).await.unwrap();
        }
        db.upsert_rule(&own_rule(6, false)).await.unwrap();
        db.upsert_rule(&preset_rule(7)).await.unwrap();
        write_config(
            dir,
            &Config {
                inbox_paths: vec!["C:\\a".into(), "C:\\b".into(), "C:\\c".into()],
            },
        )
        .unwrap();
        db
    }

    #[tokio::test]
    async fn plan_choice_is_validated_before_anything_is_written() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let db = setup(dir).await;
        let before = db.list_rules().await.unwrap();
        let trial = PlanTier::Trial { ends_at: 1 };
        let free = PlanTier::Free;
        let code = |e: CmdError| serde_json::to_value(e).unwrap();

        assert_eq!(
            code(
                apply_choice_in(&db, dir, &trial, &[], &[])
                    .await
                    .unwrap_err()
            ),
            json!("plan.notFree")
        );
        assert_eq!(
            code(
                apply_choice_in(&db, dir, &free, &[id(1), id(2), id(3), id(4)], &[])
                    .await
                    .unwrap_err()
            ),
            json!({ "code": "plan.limit", "kind": "rules", "limit": 3, "detail": "rules 3" })
        );
        let three = ["C:\\a".to_string(), "C:\\b".into(), "C:\\c".into()];
        assert_eq!(
            code(
                apply_choice_in(&db, dir, &free, &[], &three)
                    .await
                    .unwrap_err()
            ),
            json!({ "code": "plan.limit", "kind": "folders", "limit": 2, "detail": "folders 2" })
        );
        assert_eq!(
            code(
                apply_choice_in(&db, dir, &free, &[id(99)], &[])
                    .await
                    .unwrap_err()
            ),
            json!("rule.notFound")
        );
        assert_eq!(
            code(
                apply_choice_in(&db, dir, &free, &[], &["C:\\z".into()])
                    .await
                    .unwrap_err()
            ),
            json!("config.missing: C:\\z")
        );
        assert_eq!(
            serde_json::to_value(db.list_rules().await.unwrap()).unwrap(),
            serde_json::to_value(before).unwrap(),
            "nothing written"
        );
        assert_eq!(read_config(dir).unwrap().inbox_paths, three);
    }

    #[tokio::test]
    async fn plan_choice_enables_the_picked_rules_and_reorders_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let db = setup(dir).await;
        db.set_plan_choice_pending(true).await.unwrap();
        // Preset ids and duplicates do not count.
        apply_choice_in(
            &db,
            dir,
            &PlanTier::Free,
            &[id(5), id(6), id(7), id(5), id(2)],
            &["C:\\c".into(), "C:\\a".into(), "C:\\c".into()],
        )
        .await
        .unwrap();
        let rules = db.list_rules().await.unwrap();
        let get = |n| rules.iter().find(|r| r.id == id(n)).unwrap();
        for n in [2, 5, 6] {
            assert!(get(n).enabled, "r{n}");
            assert!(get(n).options.get(PLAN_DISABLED).is_none());
        }
        for n in [1, 3, 4] {
            assert!(!get(n).enabled, "r{n}");
            assert!(get(n).option_flag(PLAN_DISABLED), "r{n}");
        }
        assert!(get(7).enabled, "preset untouched");
        assert!(rules.iter().all(|r| r.version == 4), "versions kept");
        assert_eq!(
            read_config(dir).unwrap().inbox_paths,
            ["C:\\c", "C:\\a", "C:\\b"]
        );
        assert!(!db.plan_choice_pending().await.unwrap());
        // The Free limits now hold: the transition changes nothing.
        let t = crate::licensing::apply_transitions(&db, dir, &PlanTier::Free)
            .await
            .unwrap();
        assert_eq!(t.rules_changed, 0);
    }

    #[tokio::test]
    async fn export_refuses_an_existing_path_and_import_gives_fresh_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let db = setup(dir).await;
        let rules = db.list_rules().await.unwrap();
        let path = dir.join("rules.json");
        std::fs::write(&path, "keep").unwrap();
        assert_eq!(export_to(&path, &rules).unwrap_err(), ERR_EXPORT_EXISTS);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "keep",
            "not replaced"
        );

        let path = dir.join("export.json");
        export_to(&path, &rules).unwrap();
        let file: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(file["format"], "valet-rules");
        assert_eq!(file["rules"].as_array().unwrap().len(), 6, "own rules only");

        let imported = import_from(&db, &path).await.unwrap();
        assert_eq!(imported.len(), 6);
        let old: HashSet<Uuid> = rules.iter().map(|r| r.id).collect();
        assert!(imported.iter().all(|r| !old.contains(&r.id)), "fresh ids");
        assert!(imported.iter().all(|r| r.version == 1));
        assert_eq!(db.list_rules().await.unwrap().len(), 7 + 6);

        // Broken files.
        let bad = dir.join("bad.json");
        std::fs::write(&bad, "{").unwrap();
        assert_eq!(
            import_from(&db, &bad).await.unwrap_err(),
            "import.malformed"
        );
        std::fs::write(&bad, r#"{"format":"other","v":1,"rules":[]}"#).unwrap();
        assert_eq!(
            import_from(&db, &bad).await.unwrap_err(),
            "import.unknownFormat"
        );
        assert_eq!(
            import_from(&db, &dir.join("missing.json"))
                .await
                .unwrap_err(),
            ERR_IMPORT_IO
        );
    }

    #[test]
    fn export_and_import_need_trial_or_pro() {
        assert_eq!(
            require_export_import(&PlanTier::Free).unwrap_err(),
            ERR_PRO_ONLY
        );
        assert!(require_export_import(&PlanTier::Trial { ends_at: 1 }).is_ok());
    }

    #[test]
    fn oversized_license_file_is_refused_unread() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("big.valet-license");
        std::fs::write(&p, vec![b' '; (MAX_LICENSE_BYTES + 1) as usize]).unwrap();
        assert_eq!(
            read_license_file(&p).unwrap_err(),
            "license.invalid_signature"
        );
        assert_eq!(
            read_license_file(&tmp.path().join("none")).unwrap_err(),
            ERR_IO
        );
        std::fs::write(&p, "{}").unwrap();
        assert_eq!(read_license_file(&p).unwrap(), "{}");
    }
}
