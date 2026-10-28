//! Rule commands: list and upsert.

use tauri::Manager;
use valet_core::{rules::Rule, storage::Db};

use crate::state::AppState;

#[tauri::command]
pub(crate) async fn get_rules(app: tauri::AppHandle) -> Result<Vec<Rule>, String> {
    let state = app.state::<AppState>();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    db.list_rules()
        .await
        .map_err(|e| format!("Failed to list rules: {}", e))
}

#[tauri::command]
pub(crate) async fn upsert_rule(rule: Rule, app: tauri::AppHandle) -> Result<Rule, String> {
    log::info!("Upserting rule: {}", rule.name);

    let state = app.state::<AppState>();
    let db = Db::connect(&state._db_path)
        .await
        .map_err(|e| format!("Database connection failed: {}", e))?;

    // Validate rule
    if rule.name.trim().is_empty() {
        return Err("Rule name cannot be empty".to_string());
    }

    if rule.actions.is_empty() {
        return Err("Rule must have at least one action".to_string());
    }

    // Validate conditions
    for condition in &rule.conditions {
        match condition.r#type.as_str() {
            "nameMatches" => {
                if let Some(pattern) = condition.value.as_str() {
                    regex::Regex::new(pattern)
                        .map_err(|e| format!("Invalid regex pattern '{}': {}", pattern, e))?;
                } else {
                    return Err("nameMatches condition requires a string value".to_string());
                }
            }
            "ext" | "pathContains" => {
                if condition.value.as_str().is_none() {
                    return Err(format!(
                        "{} condition requires a string value",
                        condition.r#type
                    ));
                }
            }
            "sizeGt" | "sizeLt" => {
                if condition.value.as_u64().is_none() {
                    return Err(format!(
                        "{} condition requires a numeric value",
                        condition.r#type
                    ));
                }
            }
            _ => {
                return Err(format!("Unknown condition type: {}", condition.r#type));
            }
        }
    }

    // Validate actions
    for action in &rule.actions {
        match action.r#type.as_str() {
            "moveTo" | "copyTo" => {
                if let Some(path) = action.params.get("path").and_then(|v| v.as_str()) {
                    if path.trim().is_empty() {
                        return Err(format!(
                            "{} action requires a non-empty path",
                            action.r#type
                        ));
                    }
                } else {
                    return Err(format!(
                        "{} action requires a path parameter",
                        action.r#type
                    ));
                }
            }
            "tag" => {
                if action
                    .params
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .is_none()
                {
                    return Err("tag action requires a tags array parameter".to_string());
                }
            }
            "rename" => {
                if action
                    .params
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .is_none()
                {
                    return Err("rename action requires a pattern parameter".to_string());
                }
            }
            "quarantine" => {
                // No additional validation needed
            }
            _ => {
                return Err(format!("Unknown action type: {}", action.r#type));
            }
        }
    }

    db.upsert_rule(&rule)
        .await
        .map_err(|e| format!("Failed to save rule: {}", e))?;

    log::info!("Rule '{}' saved successfully", rule.name);
    Ok(rule)
}
