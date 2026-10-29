use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
