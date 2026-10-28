//! The shipped presets (PLAN.md 2.4), one JSON file each in `presets/`,
//! embedded at build time.
//!
//! # File format
//!
//! ```text
//! { id, displayOrder, defaultChecked, evalOrder,
//!   name: {ro, en}, description: {ro, en}, defaultScope: [scope entries],
//!   rules: [ { idSuffix, name: {ro, en}, conditions, actions, options, scope? } ] }
//! ```
//!
//! `conditions`, `options` and `scope` have the rule JSON shape (see
//! `apps/desktop/src-tauri/src/commands/rules.rs`); a rule without `scope`
//! gets `defaultScope`. In `actions`, `params.path` may be a `{ro, en}`
//! object, resolved for the chosen language.
//!
//! # Rule ids and order
//!
//! A preset rule's id is UUID v5 (URL namespace) of `preset:<id>:<idSuffix>`,
//! so installing a preset again updates its rules instead of adding copies.
//! The planner gives the first matching rule in storage order the file, so
//! preset rules are stored in `evalOrder` (specific before generic: invoices,
//! screenshots, old installers, downloads by type, clean desktop, large
//! files, duplicates, empty folders), and in file order within a preset.
//! [`install`] removes every installed preset rule and inserts them again in
//! that order, after all user rules: user rules are never touched and stay
//! before the preset rules, so a rule the user wrote wins over a preset.

use std::sync::OnceLock;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    rules::{Action, Condition, Rule, next_version},
    storage::Db,
};

const FILES: &[&str] = &[
    include_str!("../presets/downloads-by-type.json"),
    include_str!("../presets/screenshots.json"),
    include_str!("../presets/invoices.json"),
    include_str!("../presets/large-files.json"),
    include_str!("../presets/duplicates.json"),
    include_str!("../presets/clean-desktop.json"),
    include_str!("../presets/old-installers.json"),
    include_str!("../presets/empty-folders.json"),
];

/// UI language of preset texts and destination folder names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Ro,
    En,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Localized {
    pub ro: String,
    pub en: String,
}

impl Localized {
    pub fn get(&self, lang: Lang) -> &str {
        match lang {
            Lang::Ro => &self.ro,
            Lang::En => &self.en,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresetRule {
    pub id_suffix: String,
    pub name: Localized,
    pub conditions: Vec<Condition>,
    /// Rule actions; `params.path` may be a `{ro, en}` object.
    pub actions: Vec<Value>,
    #[serde(default)]
    pub options: Value,
    /// Overrides the preset's `defaultScope`.
    #[serde(default)]
    pub scope: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preset {
    pub id: String,
    /// Position in the onboarding list (1 = first).
    pub display_order: u32,
    /// Pre-checked in onboarding. Never true for a preset that trashes.
    pub default_checked: bool,
    /// Position of the preset's rules in storage (1 = evaluated first).
    pub eval_order: u32,
    pub name: Localized,
    pub description: Localized,
    pub default_scope: Vec<String>,
    pub rules: Vec<PresetRule>,
}

/// Stable id of preset rule `suffix` of preset `preset_id`.
pub fn rule_id(preset_id: &str, suffix: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("preset:{preset_id}:{suffix}").as_bytes(),
    )
}

/// All presets, by `displayOrder`. The embedded files are checked by tests;
/// a malformed file panics on first use.
pub fn all() -> &'static [Preset] {
    static PRESETS: OnceLock<Vec<Preset>> = OnceLock::new();
    PRESETS.get_or_init(|| {
        let mut v: Vec<Preset> = FILES
            .iter()
            .map(|f| serde_json::from_str(f).expect("embedded preset JSON"))
            .collect();
        v.sort_by_key(|p| p.display_order);
        v
    })
}

pub fn get(id: &str) -> Option<&'static Preset> {
    all().iter().find(|p| p.id == id)
}

/// All presets by `evalOrder`.
fn by_eval_order() -> Vec<&'static Preset> {
    let mut v: Vec<&Preset> = all().iter().collect();
    v.sort_by_key(|p| p.eval_order);
    v
}

/// The id belongs to a rule of a shipped preset.
pub fn is_preset_rule(id: Uuid) -> bool {
    all()
        .iter()
        .any(|p| p.rules.iter().any(|r| rule_id(&p.id, &r.id_suffix) == id))
}

/// `value` with `{ro, en}` objects at `params.path` replaced by the text.
fn localize_action(action: &Value, lang: Lang) -> Value {
    let mut a = action.clone();
    if let Some(path) = a.pointer_mut("/params/path")
        && let Some(obj) = path.as_object()
    {
        let key = match lang {
            Lang::Ro => "ro",
            Lang::En => "en",
        };
        *path = obj.get(key).cloned().unwrap_or(Value::Null);
    }
    a
}

impl Preset {
    /// The preset's rules for `lang`: enabled, version 1, stable ids.
    pub fn rules_for(&self, lang: Lang) -> Vec<Rule> {
        self.rules
            .iter()
            .map(|r| Rule {
                id: rule_id(&self.id, &r.id_suffix),
                name: r.name.get(lang).to_owned(),
                enabled: true,
                always_apply: false,
                conditions: r.conditions.clone(),
                actions: r
                    .actions
                    .iter()
                    .map(|a| {
                        serde_json::from_value::<Action>(localize_action(a, lang))
                            .expect("checked by tests")
                    })
                    .collect(),
                version: 1,
                options: if r.options.is_null() {
                    Value::Object(Default::default())
                } else {
                    r.options.clone()
                },
                scope: r
                    .scope
                    .clone()
                    .unwrap_or_else(|| self.default_scope.clone()),
            })
            .collect()
    }

    /// Distinct action types of the preset's rules, in first-use order.
    pub fn action_types(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for t in self
            .rules
            .iter()
            .flat_map(|r| &r.actions)
            .filter_map(|a| a.get("type").and_then(Value::as_str))
        {
            if !out.iter().any(|x| x == t) {
                out.push(t.to_owned());
            }
        }
        out
    }

    fn installed_in(&self, stored: &[Rule]) -> bool {
        self.rules.iter().any(|r| {
            stored
                .iter()
                .any(|s| s.id == rule_id(&self.id, &r.id_suffix))
        })
    }
}

/// The preset rules to store, in storage order, given the stored rules and
/// the presets being installed now. Presets in `install` get their shipped
/// content for `lang` (version kept when nothing changed, +1 otherwise, see
/// [`next_version`]); presets installed earlier keep their stored rules as
/// they are. Unknown ids are an error.
pub fn preset_rules_to_store(stored: &[Rule], install: &[String], lang: Lang) -> Result<Vec<Rule>> {
    for id in install {
        if get(id).is_none() {
            bail!("unknown preset {id:?}");
        }
    }
    let mut out = Vec::new();
    for p in by_eval_order() {
        if install.contains(&p.id) {
            for mut rule in p.rules_for(lang) {
                let old = stored.iter().find(|s| s.id == rule.id);
                rule.version = next_version(old, &rule);
                out.push(rule);
            }
        } else if p.installed_in(stored) {
            for r in &p.rules {
                let id = rule_id(&p.id, &r.id_suffix);
                if let Some(s) = stored.iter().find(|s| s.id == id) {
                    out.push(s.clone());
                }
            }
        }
    }
    Ok(out)
}

/// Installs or updates the presets `ids` for `lang` in one transaction (see
/// the module docs for the order) and returns their rules as stored.
pub async fn install(db: &Db, ids: &[String], lang: Lang) -> Result<Vec<Rule>> {
    let stored = db.list_rules().await?;
    let rules = preset_rules_to_store(&stored, ids, lang)?;
    db.reinsert_rules(&rules).await?;
    let wanted: Vec<Uuid> = ids
        .iter()
        .filter_map(|id| get(id))
        .flat_map(|p| p.rules.iter().map(|r| rule_id(&p.id, &r.id_suffix)))
        .collect();
    Ok(rules
        .into_iter()
        .filter(|r| wanted.contains(&r.id))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn eight_presets_parse_with_unique_orders_and_ids() {
        let p = all();
        assert_eq!(p.len(), 8);
        let display: Vec<u32> = p.iter().map(|p| p.display_order).collect();
        assert_eq!(display, (1..=8).collect::<Vec<_>>());
        let mut eval: Vec<u32> = p.iter().map(|p| p.eval_order).collect();
        eval.sort();
        assert_eq!(eval, (1..=8).collect::<Vec<_>>());
        let mut rule_ids: Vec<Uuid> = p
            .iter()
            .flat_map(|p| p.rules_for(Lang::Ro))
            .map(|r| r.id)
            .collect();
        let n = rule_ids.len();
        rule_ids.sort();
        rule_ids.dedup();
        assert_eq!(rule_ids.len(), n, "rule ids are unique");
        assert_eq!(n, 13);
    }

    #[test]
    fn display_and_eval_order_follow_the_plan() {
        let display: Vec<&str> = all().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            display,
            [
                "downloads-by-type",
                "screenshots",
                "invoices",
                "large-files",
                "duplicates",
                "clean-desktop",
                "old-installers",
                "empty-folders"
            ]
        );
        let eval: Vec<&str> = by_eval_order().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            eval,
            [
                "invoices",
                "screenshots",
                "old-installers",
                "downloads-by-type",
                "clean-desktop",
                "large-files",
                "duplicates",
                "empty-folders"
            ]
        );
        let checked: Vec<&str> = all()
            .iter()
            .filter(|p| p.default_checked)
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(
            checked,
            [
                "downloads-by-type",
                "screenshots",
                "invoices",
                "large-files"
            ]
        );
    }

    #[test]
    fn nothing_that_trashes_is_pre_checked() {
        for p in all() {
            if p.action_types().iter().any(|t| t == "trash") {
                assert!(!p.default_checked, "{}", p.id);
            }
        }
    }

    #[test]
    fn rules_are_localized_and_have_stable_ids() {
        let p = get("downloads-by-type").unwrap();
        let ro = p.rules_for(Lang::Ro);
        let en = p.rules_for(Lang::En);
        assert_eq!(ro.len(), 6);
        let paths = |rules: &[Rule]| -> Vec<String> {
            rules
                .iter()
                .map(|r| r.actions[0].params["path"].as_str().unwrap().to_owned())
                .collect()
        };
        assert_eq!(
            paths(&ro),
            [
                "Imagini",
                "Documente",
                "Arhive",
                "Instalatoare",
                "Video",
                "Audio"
            ]
        );
        assert_eq!(
            paths(&en),
            [
                "Images",
                "Documents",
                "Archives",
                "Installers",
                "Videos",
                "Music"
            ]
        );
        assert_eq!(ro[0].name, "Descărcări după tip: imagini");
        assert_eq!(en[0].name, "Downloads by type: images");
        assert_eq!(ro[0].id, en[0].id);
        assert_eq!(ro[0].id, rule_id("downloads-by-type", "image"));
        assert_eq!(
            rule_id("downloads-by-type", "image"),
            Uuid::new_v5(&Uuid::NAMESPACE_URL, b"preset:downloads-by-type:image")
        );
        assert_eq!(ro[0].scope, ["{downloads}"]);
        assert!(ro[0].option_flag("topLevelOnly"));
        assert!(is_preset_rule(ro[0].id));
        assert!(!is_preset_rule(Uuid::nil()));
        // Every localized destination is plain text after resolution.
        for p in all() {
            for lang in [Lang::Ro, Lang::En] {
                for r in p.rules_for(lang) {
                    for a in &r.actions {
                        if let Some(path) = a.params.get("path") {
                            assert!(path.is_string(), "{} {lang:?}", p.id);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn action_types_per_preset() {
        let t = |id: &str| get(id).unwrap().action_types();
        assert_eq!(t("downloads-by-type"), ["moveTo"]);
        assert_eq!(t("large-files"), ["report"]);
        assert_eq!(t("duplicates"), ["report"]);
        assert_eq!(t("old-installers"), ["trash"]);
        assert_eq!(t("empty-folders"), ["trash"]);
    }

    fn order(rules: &[Rule]) -> Vec<Uuid> {
        rules.iter().map(|r| r.id).collect()
    }

    #[test]
    fn rules_to_store_follow_eval_order_and_keep_other_presets() {
        let first =
            preset_rules_to_store(&[], &ids(&["downloads-by-type", "invoices"]), Lang::Ro).unwrap();
        assert_eq!(first.len(), 7);
        assert_eq!(first[0].id, rule_id("invoices", "move"), "invoices first");
        assert_eq!(first[1].id, rule_id("downloads-by-type", "image"));

        // Adding a preset later puts it in its place; the earlier preset rules
        // keep their stored content (here: disabled by the user).
        let mut stored = first.clone();
        stored[0].enabled = false;
        let next = preset_rules_to_store(&stored, &ids(&["screenshots"]), Lang::En).unwrap();
        assert_eq!(next.len(), 8);
        assert_eq!(next[0].id, rule_id("invoices", "move"));
        assert!(!next[0].enabled, "untouched");
        assert_eq!(next[0].name, "Facturi și chitanțe", "not re-localized");
        assert_eq!(next[1].id, rule_id("screenshots", "move"));
        assert_eq!(next[1].name, "Screenshots");
        assert_eq!(order(&next[2..]), order(&first[1..]));

        assert!(preset_rules_to_store(&[], &ids(&["nope"]), Lang::Ro).is_err());
    }

    #[test]
    fn reinstall_keeps_version_when_unchanged_and_bumps_it_otherwise() {
        let first = preset_rules_to_store(&[], &ids(&["invoices"]), Lang::Ro).unwrap();
        assert_eq!(first[0].version, 1);
        let same = preset_rules_to_store(&first, &ids(&["invoices"]), Lang::Ro).unwrap();
        assert_eq!(same[0].version, 1);
        let mut edited = first.clone();
        edited[0].scope = vec!["{desktop}".into()];
        edited[0].version = 3;
        let reset = preset_rules_to_store(&edited, &ids(&["invoices"]), Lang::Ro).unwrap();
        assert_eq!(reset[0].version, 4, "content changed back: new version");
        assert_eq!(reset[0].scope, ["{downloads}"]);
        let en = preset_rules_to_store(&first, &ids(&["invoices"]), Lang::En).unwrap();
        assert_eq!(en[0].version, 2, "other language, other content");
    }

    async fn db() -> (tempfile::TempDir, Db) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::connect(&tmp.path().join("v.sqlite3")).await.unwrap();
        (tmp, db)
    }

    fn user_rule(n: u128) -> Rule {
        serde_json::from_value(serde_json::json!({
            "id": Uuid::from_u128(n),
            "name": format!("user {n}"),
            "enabled": true,
            "conditions": [{ "type": "ext", "value": "txt" }],
            "actions": [{ "type": "report" }],
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn install_is_idempotent_and_keeps_user_rules_first() {
        let (_tmp, db) = db().await;
        db.upsert_rule(&user_rule(1)).await.unwrap();
        let installed = install(&db, &ids(&["downloads-by-type"]), Lang::Ro)
            .await
            .unwrap();
        assert_eq!(installed.len(), 6);
        db.upsert_rule(&user_rule(2)).await.unwrap();
        let again = install(
            &db,
            &ids(&["downloads-by-type", "old-installers"]),
            Lang::Ro,
        )
        .await
        .unwrap();
        assert_eq!(again.len(), 7);
        let stored = db.list_rules().await.unwrap();
        assert_eq!(stored.len(), 9, "no duplicates");
        assert_eq!(stored[0].id, Uuid::from_u128(1));
        assert_eq!(
            stored[1].id,
            Uuid::from_u128(2),
            "user rules stay before presets"
        );
        assert_eq!(stored[2].id, rule_id("old-installers", "trash"));
        assert_eq!(stored[3].id, rule_id("downloads-by-type", "image"));
        assert!(stored[2..].iter().all(|r| r.version == 1));
        let before = db.list_rules().await.unwrap();
        install(&db, &ids(&["old-installers"]), Lang::Ro)
            .await
            .unwrap();
        let after = db.list_rules().await.unwrap();
        assert_eq!(order(&before), order(&after));
        assert!(install(&db, &ids(&["nope"]), Lang::Ro).await.is_err());
        assert_eq!(db.list_rules().await.unwrap().len(), 9, "nothing written");
    }
}
