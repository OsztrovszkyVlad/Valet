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
