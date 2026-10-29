//! Plan limits (`docs/LICENSING.md`, Plans). Pure policy: the Tauri commands
//! call these and store the results. Nothing here deletes anything; rules
//! over the Free limit are disabled and marked, folders are reported
//! inactive.

use serde::{Deserialize, Serialize};

use crate::license::{DAY, PlanTier};
use crate::rules::Rule;

/// `options` key on a rule the plan disabled (as opposed to the user).
pub const PLAN_DISABLED: &str = "planDisabled";
/// Error code of [`PlanLimit`].
pub const PLAN_LIMIT_CODE: &str = "plan.limit";

pub const FREE_OWN_RULES: usize = 3;
pub const FREE_FOLDERS: usize = 2;
pub const FREE_HISTORY_DAYS: i64 = 7;

/// What a plan allows. `None` = unlimited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// Own rules (no `presetId`) enabled at once. Presets are not counted.
    pub own_rules: Option<usize>,
    /// Watched folders active at once.
    pub folders: Option<usize>,
    /// History shows batches from the last this many days.
    pub history_days: Option<i64>,
    /// Undo only the most recent batch.
    pub undo_latest_only: bool,
    pub export_import: bool,
}

impl Limits {
    pub fn for_plan(plan: &PlanTier) -> Self {
        if plan.is_free() {
            Self {
                own_rules: Some(FREE_OWN_RULES),
                folders: Some(FREE_FOLDERS),
                history_days: Some(FREE_HISTORY_DAYS),
                undo_latest_only: true,
                export_import: false,
            }
        } else {
            Self {
                own_rules: None,
                folders: None,
                history_days: None,
                undo_latest_only: false,
                export_import: true,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LimitKind {
    Rules,
    Folders,
}

/// A change refused on Free. Serializes as
/// `{ "code": "plan.limit", "kind": "rules" | "folders", "limit": n }`;
/// displays as `plan.limit: rules 3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct PlanLimit {
    pub kind: LimitKind,
    pub limit: usize,
}

impl PlanLimit {
    pub fn code(&self) -> &'static str {
        PLAN_LIMIT_CODE
    }
}

impl Serialize for PlanLimit {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("PlanLimit", 3)?;
        st.serialize_field("code", PLAN_LIMIT_CODE)?;
        st.serialize_field("kind", &self.kind)?;
        st.serialize_field("limit", &self.limit)?;
        st.end()
    }
}

impl std::fmt::Display for PlanLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self.kind {
            LimitKind::Rules => "rules",
            LimitKind::Folders => "folders",
        };
        write!(f, "{PLAN_LIMIT_CODE}: {kind} {}", self.limit)
    }
}

impl std::error::Error for PlanLimit {}

fn is_own_enabled(r: &Rule) -> bool {
    r.enabled && !r.is_preset()
}

/// May `candidate` be stored (added or replacing the rule with its id) on
/// `plan`? Refused only when it turns on an own rule (new and enabled, or
/// enabled now and not before) while `limit` other own rules are enabled.
/// Disabling, editing a rule that stays disabled or stays enabled, and
/// preset rules are always allowed.
pub fn check_rule_change(
    existing: &[Rule],
    candidate: &Rule,
    plan: &PlanTier,
) -> Result<(), PlanLimit> {
    let Some(limit) = Limits::for_plan(plan).own_rules else {
        return Ok(());
    };
    if !is_own_enabled(candidate) {
        return Ok(());
    }
    let was_own_enabled = existing
        .iter()
        .any(|r| r.id == candidate.id && is_own_enabled(r));
    if was_own_enabled {
        return Ok(());
    }
    let others = existing
        .iter()
        .filter(|r| r.id != candidate.id && is_own_enabled(r))
        .count();
    if others >= limit {
        Err(PlanLimit {
            kind: LimitKind::Rules,
            limit,
        })
    } else {
        Ok(())
    }
}

/// May the watched folders change from `existing` to `candidate` on `plan`?
/// Refused when `candidate` adds a folder (a path not in `existing`, exact
/// text) and has more than the limit. Removing and reordering are always
/// allowed (reordering picks which folders stay active).
pub fn check_folders(
    existing: &[String],
    candidate: &[String],
    plan: &PlanTier,
) -> Result<(), PlanLimit> {
    let Some(limit) = Limits::for_plan(plan).folders else {
        return Ok(());
    };
    let adds = candidate.iter().any(|p| !existing.contains(p));
    if adds && candidate.len() > limit {
        Err(PlanLimit {
            kind: LimitKind::Folders,
            limit,
        })
    } else {
        Ok(())
    }
}

fn set_plan_disabled(rule: &mut Rule, on: bool) {
    if on {
        if !rule.options.is_object() {
            rule.options = serde_json::Value::Object(Default::default());
        }
        if let Some(o) = rule.options.as_object_mut() {
            o.insert(PLAN_DISABLED.into(), serde_json::Value::Bool(true));
        }
    } else if let Some(o) = rule.options.as_object_mut() {
        o.remove(PLAN_DISABLED);
    }
}

fn has_plan_disabled_key(rule: &Rule) -> bool {
    rule.options.get(PLAN_DISABLED).is_some()
}

/// Falling back to Free: of the enabled own rules, the first 3 in the given
/// order stay enabled; the rest are disabled with `options.planDisabled =
/// true`. Pass the rules in evaluation order (`Db::list_rules`): own rules
/// come before presets there, so their relative order is the evaluation
/// order. A `planDisabled` key left on a rule that stays enabled is
/// removed. Returns the rules that changed, with `version` untouched.
/// Running it again on the result changes nothing.
pub fn apply_free_limits(rules: &[Rule]) -> Vec<Rule> {
    let mut changed = Vec::new();
    let mut kept = 0;
    for rule in rules.iter().filter(|r| is_own_enabled(r)) {
        let mut r = rule.clone();
        if kept < FREE_OWN_RULES {
            kept += 1;
            if has_plan_disabled_key(&r) {
                set_plan_disabled(&mut r, false);
                changed.push(r);
            }
        } else {
            r.enabled = false;
            set_plan_disabled(&mut r, true);
            changed.push(r);
        }
    }
    changed
}

/// Back to Trial or Pro: rules with `options.planDisabled = true` are
/// enabled again and lose the flag. Rules the user disabled (no flag) stay
/// disabled. Returns the rules that changed, with `version` untouched.
/// Idempotent.
pub fn restore_after_upgrade(rules: &[Rule]) -> Vec<Rule> {
    rules
        .iter()
        .filter(|r| has_plan_disabled_key(r))
        .map(|r| {
            let mut r = r.clone();
            if r.option_flag(PLAN_DISABLED) {
                r.enabled = true;
            }
            set_plan_disabled(&mut r, false);
            r
        })
        .collect()
}

/// `(active, inactive)` watched folders: on Free the first 2 in
/// `inbox_paths` order are active, the rest inactive. Otherwise all active.
pub fn split_active_folders(inbox_paths: &[String], plan: &PlanTier) -> (Vec<String>, Vec<String>) {
    match Limits::for_plan(plan).folders {
        Some(n) if inbox_paths.len() > n => (inbox_paths[..n].to_vec(), inbox_paths[n..].to_vec()),
        _ => (inbox_paths.to_vec(), Vec::new()),
    }
}

/// History shows only batches created at or after this time (Unix seconds):
/// `now - 7 days` on Free, no cutoff otherwise.
pub fn visible_batch_cutoff(plan: &PlanTier, now: i64) -> Option<i64> {
    Limits::for_plan(plan)
        .history_days
        .map(|d| now.saturating_sub(d * DAY))
}

/// Whether a batch created at `created_at` (the journal's
/// `YYYY-MM-DDTHH:MM:SS.fffZ` text, `BatchInfo::created_at`) is shown with
/// `cutoff` from [`visible_batch_cutoff`]. Unparseable times are shown
/// (the journal is never hidden by mistake).
pub fn batch_visible(created_at: &str, cutoff: Option<i64>) -> bool {
    let Some(cutoff) = cutoff else {
        return true;
    };
    match chrono::DateTime::parse_from_rfc3339(created_at) {
        Ok(t) => t.timestamp() >= cutoff,
        Err(_) => true,
    }
}

/// On Free only the most recent batch (`latest_batch_id`, the first of
/// `Journal::list_batches`) can be undone; otherwise any batch.
pub fn can_undo(batch_id: &str, latest_batch_id: Option<&str>, plan: &PlanTier) -> bool {
    !Limits::for_plan(plan).undo_latest_only || latest_batch_id == Some(batch_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_limit_code_display_and_json() {
        let e = PlanLimit {
            kind: LimitKind::Folders,
            limit: 2,
        };
        assert_eq!(e.code(), "plan.limit");
        assert_eq!(e.to_string(), "plan.limit: folders 2");
        assert_eq!(
            serde_json::to_value(e).unwrap(),
            serde_json::json!({ "code": "plan.limit", "kind": "folders", "limit": 2 })
        );
    }

    #[test]
    fn plan_disabled_on_non_object_options_replaces_them_with_an_object() {
        let mut r: Rule = serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::from_u128(1), "name": "r", "enabled": true,
            "conditions": [], "actions": [{ "type": "report" }], "options": null,
        }))
        .unwrap();
        set_plan_disabled(&mut r, true);
        assert!(r.option_flag(PLAN_DISABLED));
        set_plan_disabled(&mut r, false);
        assert_eq!(r.options, serde_json::json!({}));
    }

    #[test]
    fn batch_visible_compares_journal_times_with_the_cutoff() {
        // 2026-10-29T00:00:00Z = 1793232000
        let t = "2026-10-29T00:00:00.000Z";
        assert!(batch_visible(t, None));
        assert!(batch_visible(t, Some(1_793_232_000)));
        assert!(!batch_visible(t, Some(1_793_232_001)));
        assert!(batch_visible("garbage", Some(i64::MAX)));
    }
}
