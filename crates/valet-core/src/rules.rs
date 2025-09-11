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
}

fn default_version() -> i32 { 1 }

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
