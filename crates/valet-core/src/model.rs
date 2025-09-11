use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRow {
  pub id: Uuid,
  pub path: String,
  pub size: i64,
  pub mtime: i64,                 // unix seconds
  pub hash_short: Option<String>, // optional short hash
  pub tags_json: String,          // JSON array as string
}

impl FileRow {
  pub fn id_for_path(path: &str) -> Uuid {
    // deterministic, stable across re-indexes
    Uuid::new_v5(&Uuid::NAMESPACE_URL, path.as_bytes())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Op {
  MoveTo { path: String },
  CopyTo { path: String },
  Tag { tags: Vec<String> },
  Rename { pattern: String },
  Quarantine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunAction {
  pub file_path: String,
  pub op: Op,
  pub rule_id: Uuid,
  pub rule_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DryRunPlan {
  pub actions: Vec<DryRunAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
  pub total_operations: u64,
  pub successful_operations: u64,
  pub failed_operations: u64,
  pub files_moved: u64,
  pub files_copied: u64,
  pub rules_applied_count: HashMap<String, u64>,
  pub file_types_organized: HashMap<String, u64>,
  pub average_operations_per_day: f64,
  pub last_operation_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentOperation {
  pub id: u64,
  pub source_path: String,
  pub destination_path: String,
  pub operation_type: String,
  pub rule_name: String,
  pub status: String,
  pub error_message: Option<String>,
  pub created_at: String,
}