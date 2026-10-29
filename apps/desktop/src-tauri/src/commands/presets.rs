//! Preset commands: list the shipped presets, install them, finish onboarding.
//!
//! - `list_presets()` -> `[{ id, name, description, displayOrder,
//!   defaultChecked, defaultScope, ruleCount, actions }]`, by `displayOrder`,
//!   texts in the `language` of `app_settings.json`. `actions` lists the
//!   action types of the preset's rules (`moveTo`, `copyTo`, `trash`,
//!   `report`).
//! - `install_presets(presetIds)` -> the installed rules as stored. Installs
//!   or updates the rules of those presets (stable ids, so nothing is added
//!   twice); a rule whose content changed gets a new version and so needs a
//!   new preview before auto-apply runs it; identical content keeps its
//!   version. All preset rules are kept in evaluation order after the user's
//!   rules (`valet_core::presets`).
//! - `complete_onboarding(folders, presetIds)` -> `{ config, rules }`. Folder
//!   tokens (`{downloads}`, optionally followed by `/sub`) are expanded to the
//!   user's folders; the folders are validated and saved exactly like
//!   `save_config` (a folder that does not exist is saved as unavailable),
//!   then the presets are installed and `onboarding_done` is
//!   set in `app_settings.json`.
//!
//! Errors: the `config.*` codes of `save_config`, `config.missing: <entry>`
//! for an unknown folder token or one the system has no folder for, `preset.unknown: <id>`,
//! `preset.storage` (detail in the log), `settings.io`.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::{json, Value};
use valet_core::{
    planner::{PlanContext, FOLDER_TOKENS},
    presets::{self, Lang},
    rules::Rule,
    storage::Db,
};

use crate::commands::rules::connect;
use crate::commands::settings::{
    load_app_settings, save_app_settings, save_config, validate_config, AppSettings, Config,
    Language, ERR_MISSING,
};
use crate::commands::CmdError;
use crate::state::config_dir;

pub(crate) const ERR_UNKNOWN_PRESET: &str = "preset.unknown";
pub(crate) const ERR_PRESET_STORAGE: &str = "preset.storage";

fn lang(language: Language) -> Lang {
    match language {
        Language::Ro => Lang::Ro,
        Language::En => Lang::En,
    }
}

/// The presets as `list_presets` returns them.
pub(crate) fn preset_list(lang: Lang) -> Vec<Value> {
    presets::all()
        .iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name.get(lang),
                "description": p.description.get(lang),
                "displayOrder": p.display_order,
                "defaultChecked": p.default_checked,
                "defaultScope": p.default_scope,
                "ruleCount": p.rules.len(),
                "actions": p.action_types(),
            })
        })
        .collect()
}

/// `preset.unknown: <id>` for the first id that is not a shipped preset.
fn check_ids(ids: &[String]) -> Result<(), String> {
    match ids.iter().find(|id| presets::get(id).is_none()) {
        Some(id) => Err(format!("{ERR_UNKNOWN_PRESET}: {id}")),
        None => Ok(()),
    }
}

fn storage_err(e: impl std::fmt::Display) -> String {
    log::warn!("Preset storage failed: {e:#}");
    ERR_PRESET_STORAGE.to_string()
}

/// Installs `ids` for `lang` and returns their rules as stored.
pub(crate) async fn install_in(db: &Db, ids: &[String], lang: Lang) -> Result<Vec<Rule>, String> {
    check_ids(ids)?;
    presets::install(db, ids, lang).await.map_err(storage_err)
}

/// `entry` with a leading folder token replaced by its folder from
/// `folders`. `config.missing: <entry>` for an unknown token or one
/// `folders` has no folder for.
fn expand_folder(entry: &str, folders: &HashMap<String, PathBuf>) -> Result<String, String> {
    let e = entry.trim();
    let Some(after) = e.strip_prefix('{') else {
        return Ok(e.to_owned());
    };
    let missing = || format!("{ERR_MISSING}: {entry}");
    let end = after.find('}').ok_or_else(missing)?;
    let (name, rest) = (&after[..end], &after[end + 1..]);
    if !FOLDER_TOKENS.contains(&name) || !(rest.is_empty() || rest.starts_with(['/', '\\'])) {
        return Err(missing());
    }
    let base = folders.get(name).ok_or_else(missing)?;
    let rest = rest.trim_start_matches(['/', '\\']);
    let path = if rest.is_empty() {
        base.clone()
    } else {
        base.join(rest)
    };
    Ok(path.to_string_lossy().into_owned())
}

/// The config onboarding saves: tokens expanded, duplicates dropped,
/// validated like `save_config`.
pub(crate) fn onboarding_config(
    folders: &[String],
    known: &HashMap<String, PathBuf>,
) -> Result<Config, String> {
    let mut inbox_paths: Vec<String> = Vec::new();
    for f in folders {
        let p = expand_folder(f, known)?;
        if !inbox_paths.contains(&p) {
            inbox_paths.push(p);
        }
    }
    let config = Config { inbox_paths };
    validate_config(&config)?;
    Ok(config)
}

#[tauri::command]
pub(crate) async fn list_presets(app: tauri::AppHandle) -> Result<Vec<Value>, String> {
    let settings = load_app_settings(&config_dir(&app)?);
    Ok(preset_list(lang(settings.language)))
}

#[tauri::command]
pub(crate) async fn install_presets(
    preset_ids: Vec<String>,
    app: tauri::AppHandle,
) -> Result<Vec<Rule>, String> {
    let settings = load_app_settings(&config_dir(&app)?);
    let db = connect(&app).await?;
    let rules = install_in(&db, &preset_ids, lang(settings.language)).await?;
    log::info!(
        "Presets installed: {} preset(s), {} rule(s)",
        preset_ids.len(),
        rules.len()
    );
    Ok(rules)
}

#[derive(Serialize)]
pub(crate) struct Onboarding {
    config: Config,
    rules: Vec<Rule>,
}

#[tauri::command]
pub(crate) async fn complete_onboarding(
    folders: Vec<String>,
    preset_ids: Vec<String>,
    app: tauri::AppHandle,
) -> Result<Onboarding, CmdError> {
    let dir = config_dir(&app)?;
    // Everything is checked before anything is written.
    let config = onboarding_config(&folders, &PlanContext::from_system().folders)?;
    check_ids(&preset_ids)?;
    let db = connect(&app).await?;

    let config = save_config(config, app.clone()).await?;
    let settings = load_app_settings(&dir);
    let rules = install_in(&db, &preset_ids, lang(settings.language)).await?;
    save_app_settings(
        AppSettings {
            onboarding_done: true,
            ..settings
        },
        app,
    )
    .await?;
    log::info!(
        "Onboarding done: {} folder(s), {} preset(s)",
        config.inbox_paths.len(),
        preset_ids.len()
    );
    Ok(Onboarding { config, rules })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::rules::validate_rule;
    use crate::commands::settings::{ERR_DRIVE_ROOT, ERR_EXCLUDED_FOLDER};
    use std::path::Path;

    #[test]
    fn every_preset_rule_passes_rule_validation_in_both_languages() {
        for p in presets::all() {
            for l in [Lang::Ro, Lang::En] {
                for r in p.rules_for(l) {
                    assert_eq!(validate_rule(&r), Ok(()), "{} {:?} {}", p.id, l, r.name);
                }
            }
        }
    }

    #[test]
    fn list_is_localized_and_ordered() {
        let ro = preset_list(Lang::Ro);
        let en = preset_list(Lang::En);
        assert_eq!(ro.len(), 8);
        assert_eq!(ro[0]["id"], "downloads-by-type");
        assert_eq!(ro[0]["name"], "Descărcări după tip");
        assert_eq!(en[0]["name"], "Downloads by type");
        assert_eq!(ro[0]["displayOrder"], 1);
        assert_eq!(ro[0]["defaultChecked"], true);
        assert_eq!(ro[0]["defaultScope"], json!(["{downloads}"]));
        assert_eq!(ro[0]["ruleCount"], 6);
        assert_eq!(ro[0]["actions"], json!(["moveTo"]));
        let last = &ro[7];
        assert_eq!(last["id"], "empty-folders");
        assert_eq!(last["defaultChecked"], false);
        assert_eq!(last["actions"], json!(["trash"]));
        assert_ne!(ro[3]["description"], en[3]["description"]);
    }

    async fn db() -> (tempfile::TempDir, Db) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("valet.sqlite3"))
            .await
            .unwrap();
        (tmp, db)
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[tokio::test]
    async fn install_is_idempotent_and_ordered() {
        let (_tmp, db) = db().await;
        let first = install_in(&db, &ids(&["downloads-by-type", "invoices"]), Lang::Ro)
            .await
            .unwrap();
        assert_eq!(first.len(), 7);
        assert_eq!(first[0].name, "Facturi și chitanțe", "evaluated first");
        assert_eq!(first[0].options["presetId"], "invoices");
        assert_eq!(first[1].options["presetId"], "downloads-by-type");
        let again = install_in(&db, &ids(&["invoices", "downloads-by-type"]), Lang::Ro)
            .await
            .unwrap();
        let ids_of = |r: &[Rule]| r.iter().map(|r| (r.id, r.version)).collect::<Vec<_>>();
        assert_eq!(ids_of(&again), ids_of(&first), "same ids and versions");
        assert_eq!(db.list_rules().await.unwrap().len(), 7);
    }

    #[tokio::test]
    async fn reinstall_with_changed_content_bumps_the_version() {
        let (_tmp, db) = db().await;
        let first = install_in(&db, &ids(&["invoices"]), Lang::Ro)
            .await
            .unwrap();
        assert_eq!(first[0].version, 1);
        // The user edits the preset rule (version 2), then reinstalls it.
        let mut edited = first[0].clone();
        edited.scope = vec!["{desktop}".into()];
        crate::commands::rules::save_rule(&db, edited)
            .await
            .unwrap();
        let back = install_in(&db, &ids(&["invoices"]), Lang::Ro)
            .await
            .unwrap();
        assert_eq!(back[0].version, 3, "content changed: new preview needed");
        assert_eq!(back[0].scope, ["{downloads}"]);
        let same = install_in(&db, &ids(&["invoices"]), Lang::Ro)
            .await
            .unwrap();
        assert_eq!(same[0].version, 3, "identical content keeps the version");
        let en = install_in(&db, &ids(&["invoices"]), Lang::En)
            .await
            .unwrap();
        assert_eq!(en[0].version, 4);
        assert_eq!(en[0].name, "Invoices and receipts");
    }

    #[tokio::test]
    async fn unknown_preset_is_refused_and_nothing_written() {
        let (_tmp, db) = db().await;
        let err = install_in(&db, &ids(&["invoices", "nope"]), Lang::Ro)
            .await
            .unwrap_err();
        assert_eq!(err, "preset.unknown: nope");
        assert!(db.list_rules().await.unwrap().is_empty());
    }

    #[test]
    fn onboarding_expands_tokens_and_validates_like_save_config() {
        // None of these folders exists; like `save_config`, onboarding does
        // not refuse a missing folder.
        let home = Path::new(env!("CARGO_MANIFEST_DIR")).join("valet-test-home");
        let downloads = home.join("Downloads");
        let desktop = home.join("Desktop");
        let known: HashMap<String, PathBuf> = [
            ("downloads".to_string(), downloads.clone()),
            ("desktop".to_string(), desktop.clone()),
            ("pictures".to_string(), home.join("Pictures")),
        ]
        .into_iter()
        .collect();
        let s = |p: &Path| p.to_string_lossy().into_owned();

        let c = onboarding_config(
            &ids(&["{downloads}", "{desktop}", "{downloads}/sub", "{downloads}"]),
            &known,
        )
        .unwrap();
        assert_eq!(
            c.inbox_paths,
            [s(&downloads), s(&desktop), s(&downloads.join("sub"))]
        );
        // Absolute paths pass through.
        let c = onboarding_config(&[s(&desktop)], &known).unwrap();
        assert_eq!(c.inbox_paths, [s(&desktop)]);

        for (entry, code) in [
            ("{nope}", "config.missing: {nope}"),
            ("{music}", "config.missing: {music}"),
            ("{downloads}x", "config.missing: {downloads}x"),
        ] {
            assert_eq!(onboarding_config(&ids(&[entry]), &known).unwrap_err(), code);
        }
        // A known token whose folder does not exist is saved (unavailable).
        let c = onboarding_config(&ids(&["{pictures}"]), &known).unwrap();
        assert_eq!(c.inbox_paths, [s(&home.join("Pictures"))]);
        assert!(onboarding_config(&ids(&["C:\\"]), &known)
            .unwrap_err()
            .starts_with(ERR_DRIVE_ROOT));
        assert!(
            onboarding_config(&ids(&["C:\\Users\\Ana\\AppData\\Local"]), &known)
                .unwrap_err()
                .starts_with(ERR_EXCLUDED_FOLDER)
        );
    }
}
