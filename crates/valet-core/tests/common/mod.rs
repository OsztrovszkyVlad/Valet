//! Small helpers shared by the characterization tests.
#![allow(dead_code)]

use std::fs;
use std::path::Path;

use serde_json::{Value, json};
use uuid::Uuid;
use valet_core::rules::Rule;

/// Builds a `Rule` from JSON in the same shape the app stores it.
/// `n` gives a deterministic id (`Uuid::from_u128(n)`).
pub fn rule(n: u128, name: &str, conditions: Value, actions: Value) -> Rule {
    serde_json::from_value(json!({
        "id": Uuid::from_u128(n),
        "name": name,
        "enabled": true,
        "conditions": conditions,
        "actions": actions,
    }))
    .expect("rule json must deserialize")
}

/// Rule with a single condition and a single `tag` action.
pub fn cond_rule(cond_type: &str, value: Value) -> Rule {
    rule(
        1,
        "r",
        json!([{ "type": cond_type, "value": value }]),
        json!([{ "type": "tag", "params": { "tags": ["t"] } }]),
    )
}

/// Writes `len` bytes to `path`, creating parent directories.
pub fn write_file(path: &Path, len: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, vec![b'x'; len]).expect("write file");
}

/// Path as the `String` the engine produces (`to_string_lossy`).
pub fn s(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
