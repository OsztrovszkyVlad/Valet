//! Rule model. The JSON schema the app accepts (condition and action types
//! with their value types) is documented and validated in
//! `apps/desktop/src-tauri/src/commands/rules.rs`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    #[serde(rename = "alwaysApply", default)]
    pub always_apply: bool,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    #[serde(default = "default_version")]
    pub version: i32,
    #[serde(default)]
    pub options: serde_json::Value,
    /// Watched folders the rule applies to: folder tokens (`{downloads}`) or
    /// absolute paths. Empty: every watched folder.
    #[serde(default)]
    pub scope: Vec<String>,
}

impl Rule {
    /// `options.<key>` is `true`.
    pub fn option_flag(&self, key: &str) -> bool {
        self.options.get(key).and_then(serde_json::Value::as_bool) == Some(true)
    }

    /// Equal in every field except `version`.
    pub fn same_content(&self, other: &Rule) -> bool {
        let strip = |r: &Rule| {
            let mut v = serde_json::to_value(r).unwrap_or_default();
            if let Some(o) = v.as_object_mut() {
                o.remove("version");
            }
            v
        };
        strip(self) == strip(other)
    }
}

/// Version to store for `incoming`: a new rule keeps its own version (at
/// least 1); a stored rule keeps its version when nothing else changed and
/// gets the stored version + 1 otherwise. The version the caller sends for an
/// existing rule is ignored.
pub fn next_version(stored: Option<&Rule>, incoming: &Rule) -> i32 {
    match stored {
        None => incoming.version.max(1),
        Some(s) if s.same_content(incoming) => s.version,
        Some(s) => s.version.saturating_add(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rule(name: &str, version: i32) -> Rule {
        serde_json::from_value(json!({
            "id": "00000000-0000-0000-0000-000000000001",
            "name": name,
            "enabled": true,
            "conditions": [{ "type": "ext", "value": "pdf" }],
            "actions": [{ "type": "report" }],
            "version": version,
        }))
        .unwrap()
    }

    #[test]
    fn new_rule_keeps_its_version_at_least_one() {
        assert_eq!(next_version(None, &rule("a", 3)), 3);
        assert_eq!(next_version(None, &rule("a", 0)), 1);
        assert_eq!(next_version(None, &rule("a", -2)), 1);
    }

    #[test]
    fn unchanged_rule_keeps_stored_version_whatever_the_caller_sends() {
        let stored = rule("a", 4);
        assert_eq!(next_version(Some(&stored), &rule("a", 4)), 4);
        assert_eq!(next_version(Some(&stored), &rule("a", 1)), 4);
        assert_eq!(next_version(Some(&stored), &rule("a", 99)), 4);
    }

    #[test]
    fn any_content_change_bumps_the_stored_version() {
        let stored = rule("a", 4);
        assert_eq!(next_version(Some(&stored), &rule("b", 1)), 5);
        let mut scoped = rule("a", 4);
        scoped.scope = vec!["{desktop}".into()];
        assert_eq!(next_version(Some(&stored), &scoped), 5);
        let mut disabled = rule("a", 4);
        disabled.enabled = false;
        assert_eq!(next_version(Some(&stored), &disabled), 5);
        let mut opts = rule("a", 4);
        opts.options = json!({ "topLevelOnly": true });
        assert_eq!(next_version(Some(&stored), &opts), 5);
        assert_eq!(
            next_version(Some(&rule("a", i32::MAX)), &rule("b", 1)),
            i32::MAX
        );
    }

    #[test]
    fn option_flag_is_true_only_for_boolean_true() {
        let mut r = rule("a", 1);
        assert!(!r.option_flag("topLevelOnly"));
        r.options = json!({ "topLevelOnly": "true", "includeHidden": true });
        assert!(!r.option_flag("topLevelOnly"));
        assert!(r.option_flag("includeHidden"));
    }
}

fn default_version() -> i32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub r#type: String,
    #[serde(default)]
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub r#type: String,
    #[serde(default)]
    pub params: serde_json::Value,
}
