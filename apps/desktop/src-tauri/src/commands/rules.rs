//! Rule commands: CRUD, templates, import/export, validation, analytics.

use std::path::Path;

use tauri::Manager;
use valet_core::{rules::Rule, storage::Db};

use crate::state::AppState;

// Rule Template System
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub(crate) struct RuleTemplate {
  id: String,
  name: String,
  description: String,
  category: TemplateCategory,
  parameters: Vec<TemplateParameter>,
  rule_pattern: String, // Template string with placeholders
  example_usage: String,
  tags: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
enum TemplateCategory {
  FileType,
  DateBased,
  ProjectStructure,
  ContentBased,
  SizeBased,
  Custom,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct TemplateParameter {
  name: String,
  param_type: ParameterType,
  description: String,
  default_value: Option<String>,
  required: bool,
  validation_regex: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
enum ParameterType {
  String,
  Number,
  Boolean,
  FileExtension,
  DateFormat,
  Path,
  RegexPattern,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct RuleFromTemplate {
  template_id: String,
  parameters: std::collections::HashMap<String, String>,
  custom_name: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RuleExport {
  rules: Vec<Rule>,
  templates: Vec<RuleTemplate>,
  exported_at: String,
  version: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct RuleValidationResult {
  is_valid: bool,
  errors: Vec<String>,
  warnings: Vec<String>,
  affected_file_count: Option<u32>,
  preview_actions: Vec<RulePreview>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RulePreview {
  source_path: String,
  destination_path: String,
  action_type: String,
  confidence: f32,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct RuleAnalytics {
  rule_id: u32,
  usage_count: u32,
  success_rate: f32,
  avg_files_processed: f32,
  last_used: String,
  effectiveness_score: f32,
}

// Predefined Rule Templates
fn get_predefined_templates() -> Vec<RuleTemplate> {
  vec![
    RuleTemplate {
      id: "file-type-organizer".to_string(),
      name: "File Type Organizer".to_string(),
      description: "Organizes files by their extension into categorized folders".to_string(),
      category: TemplateCategory::FileType,
      parameters: vec![
        TemplateParameter {
          name: "base_path".to_string(),
          param_type: ParameterType::Path,
          description: "Base directory where organized folders will be created".to_string(),
          default_value: Some("./Organized".to_string()),
          required: true,
          validation_regex: None,
        },
        TemplateParameter {
          name: "file_extensions".to_string(),
          param_type: ParameterType::String,
          description: "Comma-separated list of file extensions to organize".to_string(),
          default_value: Some("jpg,png,pdf,docx,txt".to_string()),
          required: true,
          validation_regex: Some(r"^[a-zA-Z0-9,]+$".to_string()),
        }
      ],
      rule_pattern: "if extension in [{file_extensions}] then move to {base_path}/{extension_category}/".to_string(),
      example_usage: "Organizes images to ./Organized/Images/, documents to ./Organized/Documents/".to_string(),
      tags: vec!["organization".to_string(), "file-type".to_string(), "automatic".to_string()],
    },
    RuleTemplate {
      id: "date-based-archiver".to_string(),
      name: "Date-Based File Archiver".to_string(),
      description: "Archives files into folders based on their creation or modification date".to_string(),
      category: TemplateCategory::DateBased,
      parameters: vec![
        TemplateParameter {
          name: "date_format".to_string(),
          param_type: ParameterType::DateFormat,
          description: "Date format for folder structure (e.g., YYYY/MM, YYYY-MM-DD)".to_string(),
          default_value: Some("YYYY/MM".to_string()),
          required: true,
          validation_regex: None,
        },
        TemplateParameter {
          name: "archive_path".to_string(),
          param_type: ParameterType::Path,
          description: "Base path for date-organized archives".to_string(),
          default_value: Some("./Archive".to_string()),
          required: true,
          validation_regex: None,
        },
        TemplateParameter {
          name: "date_source".to_string(),
          param_type: ParameterType::String,
          description: "Use 'created' or 'modified' date for organization".to_string(),
          default_value: Some("modified".to_string()),
          required: true,
          validation_regex: Some("^(created|modified)$".to_string()),
        }
      ],
      rule_pattern: "if {date_source}_date then move to {archive_path}/{date_format}/".to_string(),
      example_usage: "Files modified in Jan 2024 go to ./Archive/2024/01/".to_string(),
      tags: vec!["archive".to_string(), "date".to_string(), "backup".to_string()],
    },
    RuleTemplate {
      id: "project-structure".to_string(),
      name: "Project Structure Organizer".to_string(),
      description: "Organizes files into a standard project structure based on file types and patterns".to_string(),
      category: TemplateCategory::ProjectStructure,
      parameters: vec![
        TemplateParameter {
          name: "project_name".to_string(),
          param_type: ParameterType::String,
          description: "Name of the project for the base folder".to_string(),
          default_value: Some("MyProject".to_string()),
          required: true,
          validation_regex: Some(r"^[a-zA-Z0-9_-]+$".to_string()),
        },
        TemplateParameter {
          name: "include_assets".to_string(),
          param_type: ParameterType::Boolean,
          description: "Create separate folders for assets (images, videos, etc.)".to_string(),
          default_value: Some("true".to_string()),
          required: false,
          validation_regex: None,
        }
      ],
      rule_pattern: "if code_file then move to {project_name}/src/ else if asset_file and {include_assets} then move to {project_name}/assets/ else move to {project_name}/docs/".to_string(),
      example_usage: "Code files → MyProject/src/, Images → MyProject/assets/, Others → MyProject/docs/".to_string(),
      tags: vec!["project".to_string(), "development".to_string(), "structure".to_string()],
    },
    RuleTemplate {
      id: "size-based-sorter".to_string(),
      name: "File Size Based Sorter".to_string(),
      description: "Sorts files into folders based on their file size ranges".to_string(),
      category: TemplateCategory::SizeBased,
      parameters: vec![
        TemplateParameter {
          name: "small_threshold_mb".to_string(),
          param_type: ParameterType::Number,
          description: "Maximum size in MB for small files".to_string(),
          default_value: Some("1".to_string()),
          required: true,
          validation_regex: Some(r"^\d+$".to_string()),
        },
        TemplateParameter {
          name: "large_threshold_mb".to_string(),
          param_type: ParameterType::Number,
          description: "Minimum size in MB for large files".to_string(),
          default_value: Some("100".to_string()),
          required: true,
          validation_regex: Some(r"^\d+$".to_string()),
        },
        TemplateParameter {
          name: "sort_path".to_string(),
          param_type: ParameterType::Path,
          description: "Base path for size-sorted folders".to_string(),
          default_value: Some("./SortedBySize".to_string()),
          required: true,
          validation_regex: None,
        }
      ],
      rule_pattern: "if size < {small_threshold_mb}MB then move to {sort_path}/Small/ else if size > {large_threshold_mb}MB then move to {sort_path}/Large/ else move to {sort_path}/Medium/".to_string(),
      example_usage: "Files <1MB → Small/, >100MB → Large/, others → Medium/".to_string(),
      tags: vec!["size".to_string(), "storage".to_string(), "optimization".to_string()],
    },
    RuleTemplate {
      id: "content-classifier".to_string(),
      name: "Content-Based Classifier".to_string(),
      description: "Classifies files based on content patterns and keywords in filenames".to_string(),
      category: TemplateCategory::ContentBased,
      parameters: vec![
        TemplateParameter {
          name: "work_keywords".to_string(),
          param_type: ParameterType::String,
          description: "Keywords that indicate work-related files".to_string(),
          default_value: Some("work,business,project,meeting,report".to_string()),
          required: true,
          validation_regex: None,
        },
        TemplateParameter {
          name: "personal_keywords".to_string(),
          param_type: ParameterType::String,
          description: "Keywords that indicate personal files".to_string(),
          default_value: Some("personal,family,vacation,photo,hobby".to_string()),
          required: true,
          validation_regex: None,
        },
        TemplateParameter {
          name: "classifier_path".to_string(),
          param_type: ParameterType::Path,
          description: "Base path for classified folders".to_string(),
          default_value: Some("./Classified".to_string()),
          required: true,
          validation_regex: None,
        }
      ],
      rule_pattern: "if filename contains [{work_keywords}] then move to {classifier_path}/Work/ else if filename contains [{personal_keywords}] then move to {classifier_path}/Personal/ else move to {classifier_path}/Uncategorized/".to_string(),
      example_usage: "meeting-notes.pdf → Work/, family-photo.jpg → Personal/".to_string(),
      tags: vec!["content".to_string(), "classification".to_string(), "keywords".to_string()],
    }
  ]
}

// Tauri Commands
#[tauri::command]
pub(crate) async fn get_rules(app: tauri::AppHandle) -> Result<Vec<Rule>, String> {
  let state = app.state::<AppState>();
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;
  
  db.list_rules().await
    .map_err(|e| format!("Failed to list rules: {}", e))
}

#[tauri::command]
pub(crate) async fn upsert_rule(rule: Rule, app: tauri::AppHandle) -> Result<Rule, String> {
  log::info!("Upserting rule: {}", rule.name);
  
  let state = app.state::<AppState>();
  let db = Db::connect(&state._db_path).await
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
          return Err(format!("{} condition requires a string value", condition.r#type));
        }
      }
      "sizeGt" | "sizeLt" => {
        if condition.value.as_u64().is_none() {
          return Err(format!("{} condition requires a numeric value", condition.r#type));
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
            return Err(format!("{} action requires a non-empty path", action.r#type));
          }
        } else {
          return Err(format!("{} action requires a path parameter", action.r#type));
        }
      }
      "tag" => {
        if action.params.get("tags").and_then(|v| v.as_array()).is_none() {
          return Err("tag action requires a tags array parameter".to_string());
        }
      }
      "rename" => {
        if action.params.get("pattern").and_then(|v| v.as_str()).is_none() {
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
  
  db.upsert_rule(&rule).await
    .map_err(|e| format!("Failed to save rule: {}", e))?;
  
  log::info!("Rule '{}' saved successfully", rule.name);
  Ok(rule)
}

// Rule Template Commands
#[tauri::command]
pub(crate) async fn get_rule_templates() -> Result<Vec<RuleTemplate>, String> {
  Ok(get_predefined_templates())
}

#[tauri::command]
pub(crate) async fn create_rule_from_template(template_request: RuleFromTemplate, app: tauri::AppHandle) -> Result<Rule, String> {
  let templates = get_predefined_templates();
  let template = templates.iter()
    .find(|t| t.id == template_request.template_id)
    .ok_or_else(|| format!("Template '{}' not found", template_request.template_id))?;

  // Validate all required parameters are provided
  for param in &template.parameters {
    if param.required && !template_request.parameters.contains_key(&param.name) {
      return Err(format!("Required parameter '{}' not provided", param.name));
    }
  }

  // Validate parameter values against regex if specified
  for (param_name, param_value) in &template_request.parameters {
    if let Some(param_def) = template.parameters.iter().find(|p| p.name == *param_name) {
      if let Some(regex_pattern) = &param_def.validation_regex {
        let regex = regex::Regex::new(regex_pattern)
          .map_err(|e| format!("Invalid validation regex for parameter '{}': {}", param_name, e))?;
        if !regex.is_match(param_value) {
          return Err(format!("Parameter '{}' value '{}' doesn't match required pattern", param_name, param_value));
        }
      }
    }
  }

  // Create rule from template
  let rule_name = template_request.custom_name
    .unwrap_or_else(|| format!("{} - {}", template.name, chrono::Utc::now().format("%Y%m%d_%H%M%S")));

  // Generate rule conditions and actions based on template pattern
  let conditions = generate_conditions_from_template(template, &template_request.parameters)?;
  let actions = generate_actions_from_template(template, &template_request.parameters)?;

  let rule = Rule {
    id: uuid::Uuid::new_v4(),
    name: rule_name,
    conditions,
    actions,
    enabled: true,
    always_apply: false,
    version: 1,
    options: serde_json::Value::Null,
  };

  // Save the generated rule
  upsert_rule(rule, app).await
}

#[tauri::command]
pub(crate) async fn export_rules(rule_ids: Vec<u32>, include_templates: bool, app: tauri::AppHandle) -> Result<String, String> {
  let state = app.state::<AppState>();
  let db = Db::connect(&state._db_path).await
    .map_err(|e| format!("Database connection failed: {}", e))?;

  // Get selected rules
  let all_rules = db.list_rules().await
    .map_err(|e| format!("Failed to list rules: {}", e))?;
    
  let mut rules = Vec::new();
  for rule_id in rule_ids {
    if let Some(rule) = all_rules.iter().find(|r| r.id.to_string() == rule_id.to_string()) {
      rules.push(rule.clone());
    }
  }

  let export = RuleExport {
    rules,
    templates: if include_templates { get_predefined_templates() } else { Vec::new() },
    exported_at: chrono::Utc::now().to_rfc3339(),
    version: "1.0".to_string(),
  };

  serde_json::to_string_pretty(&export)
    .map_err(|e| format!("Failed to serialize export: {}", e))
}

#[tauri::command]
pub(crate) async fn import_rules(import_data: String, overwrite_existing: bool, app: tauri::AppHandle) -> Result<Vec<String>, String> {
  let import: RuleExport = serde_json::from_str(&import_data)
    .map_err(|e| format!("Failed to parse import data: {}", e))?;

  let mut imported_rules = Vec::new();
  let mut errors = Vec::new();

  for mut rule in import.rules {
    // Reset ID for import (will be assigned new ID)
    let original_name = rule.name.clone();
    rule.id = uuid::Uuid::new_v4();
    
    // Check if rule with same name exists
    if !overwrite_existing {
      rule.name = format!("{} (imported)", rule.name);
    }

    match upsert_rule(rule, app.clone()).await {
      Ok(saved_rule) => imported_rules.push(saved_rule.name),
      Err(e) => errors.push(format!("Failed to import '{}': {}", original_name, e)),
    }
  }

  if !errors.is_empty() {
    Err(format!("Import completed with errors: {}", errors.join("; ")))
  } else {
    Ok(imported_rules)
  }
}

#[tauri::command]
pub(crate) async fn validate_rule(rule: Rule, test_path: Option<String>, _app: tauri::AppHandle) -> Result<RuleValidationResult, String> {
  let mut errors = Vec::new();
  let mut warnings = Vec::new();

  // Validate rule structure
  if rule.name.trim().is_empty() {
    errors.push("Rule name cannot be empty".to_string());
  }

  if rule.conditions.is_empty() {
    errors.push("Rule must have at least one condition".to_string());
  }

  if rule.actions.is_empty() {
    errors.push("Rule must have at least one action".to_string());
  }

  // Validate conditions
  for (i, condition) in rule.conditions.iter().enumerate() {
    match condition.r#type.as_str() {
      "nameMatches" => {
        if let Some(pattern) = condition.value.as_str() {
          if let Err(e) = regex::Regex::new(pattern) {
            errors.push(format!("Condition {}: Invalid regex pattern '{}': {}", i + 1, pattern, e));
          }
        }
      },
      "extension" => {
        if let Some(ext) = condition.value.as_str() {
          if ext.contains('.') {
            warnings.push(format!("Condition {}: Extension '{}' should not include dots", i + 1, ext));
          }
        }
      },
      _ => {
        warnings.push(format!("Condition {}: Unknown condition type '{}'", i + 1, condition.r#type));
      }
    }
  }

  // Test rule against path if provided
  let mut preview_actions = Vec::new();
  let mut affected_count = 0;

  if let Some(path) = test_path {
    match test_rule_against_path(&rule, &path).await {
      Ok(previews) => {
        affected_count = previews.len() as u32;
        preview_actions = previews;
      },
      Err(e) => warnings.push(format!("Could not test rule against path: {}", e)),
    }
  }

  Ok(RuleValidationResult {
    is_valid: errors.is_empty(),
    errors,
    warnings,
    affected_file_count: if affected_count > 0 { Some(affected_count) } else { None },
    preview_actions,
  })
}

#[tauri::command]
pub(crate) async fn get_rule_analytics(rule_id: Option<u32>, _app: tauri::AppHandle) -> Result<Vec<RuleAnalytics>, String> {
  // This would integrate with a usage tracking system
  // For now, return mock data
  Ok(vec![
    RuleAnalytics {
      rule_id: rule_id.unwrap_or(1),
      usage_count: 42,
      success_rate: 0.95,
      avg_files_processed: 15.3,
      last_used: chrono::Utc::now().to_rfc3339(),
      effectiveness_score: 0.87,
    }
  ])
}

// Helper functions for rule templates
fn generate_conditions_from_template(template: &RuleTemplate, parameters: &std::collections::HashMap<String, String>) -> Result<Vec<valet_core::rules::Condition>, String> {
  use valet_core::rules::Condition;
  
  let mut conditions = Vec::new();
  
  match template.category {
    TemplateCategory::FileType => {
      if let Some(extensions) = parameters.get("file_extensions") {
        let exts: Vec<&str> = extensions.split(',').collect();
        for ext in exts {
          conditions.push(Condition {
            r#type: "extension".to_string(),
            value: serde_json::Value::String(ext.trim().to_string()),
          });
        }
      }
    },
    TemplateCategory::DateBased => {
      conditions.push(Condition {
        r#type: "hasDate".to_string(),
        value: serde_json::Value::Bool(true),
      });
    },
    TemplateCategory::SizeBased => {
      if let Some(small_threshold) = parameters.get("small_threshold_mb") {
        conditions.push(Condition {
          r#type: "sizeRange".to_string(),
          value: serde_json::Value::String(format!("0-{}MB", small_threshold)),
        });
      }
    },
    TemplateCategory::ContentBased => {
      if let Some(keywords) = parameters.get("work_keywords") {
        conditions.push(Condition {
          r#type: "nameMatches".to_string(),
          value: serde_json::Value::String(format!(".*({}).*", keywords.replace(',', "|"))),
        });
      }
    },
    _ => {
      conditions.push(Condition {
        r#type: "always".to_string(),
        value: serde_json::Value::Bool(true),
      });
    }
  }
  
  Ok(conditions)
}

fn generate_actions_from_template(template: &RuleTemplate, parameters: &std::collections::HashMap<String, String>) -> Result<Vec<valet_core::rules::Action>, String> {
  use valet_core::rules::Action;
  
  let mut actions = Vec::new();
  
  match template.category {
    TemplateCategory::FileType => {
      if let Some(base_path) = parameters.get("base_path") {
        actions.push(Action {
          r#type: "move".to_string(),
          params: serde_json::json!({ "target": format!("{}/{{extension}}/", base_path) }),
        });
      }
    },
    TemplateCategory::DateBased => {
      if let Some(archive_path) = parameters.get("archive_path") {
        let default_format = "YYYY/MM".to_string();
        let date_format = parameters.get("date_format").unwrap_or(&default_format);
        actions.push(Action {
          r#type: "move".to_string(),
          params: serde_json::json!({ "target": format!("{}/{}/", archive_path, date_format) }),
        });
      }
    },
    TemplateCategory::ProjectStructure => {
      if let Some(project_name) = parameters.get("project_name") {
        actions.push(Action {
          r#type: "move".to_string(),
          params: serde_json::json!({ "target": format!("{}/src/", project_name) }),
        });
      }
    },
    TemplateCategory::SizeBased => {
      if let Some(sort_path) = parameters.get("sort_path") {
        actions.push(Action {
          r#type: "move".to_string(),
          params: serde_json::json!({ "target": format!("{}/{{size_category}}/", sort_path) }),
        });
      }
    },
    TemplateCategory::ContentBased => {
      if let Some(classifier_path) = parameters.get("classifier_path") {
        actions.push(Action {
          r#type: "move".to_string(),
          params: serde_json::json!({ "target": format!("{}/{{content_category}}/", classifier_path) }),
        });
      }
    },
    _ => {
      actions.push(Action {
        r#type: "move".to_string(),
        params: serde_json::json!({ "target": "./Organized/" }),
      });
    }
  }
  
  Ok(actions)
}

async fn test_rule_against_path(rule: &valet_core::rules::Rule, test_path: &str) -> Result<Vec<RulePreview>, String> {
  use std::path::Path;
  use std::fs;
  
  let path = Path::new(test_path);
  if !path.exists() {
    return Err("Test path does not exist".to_string());
  }
  
  let mut previews = Vec::new();
  
  if path.is_file() {
    // Test single file
    if let Some(preview) = simulate_rule_on_file(rule, path)? {
      previews.push(preview);
    }
  } else if path.is_dir() {
    // Test directory contents
    let entries = fs::read_dir(path)
      .map_err(|e| format!("Failed to read directory: {}", e))?;
      
    for entry in entries {
      if let Ok(entry) = entry {
        let file_path = entry.path();
        if file_path.is_file() {
          if let Some(preview) = simulate_rule_on_file(rule, &file_path)? {
            previews.push(preview);
          }
        }
      }
    }
  }
  
  Ok(previews)
}

fn simulate_rule_on_file(rule: &valet_core::rules::Rule, file_path: &Path) -> Result<Option<RulePreview>, String> {
  // This is a simplified simulation - in a real implementation,
  // this would use the actual rule engine logic
  
  let file_name = file_path.file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("");
    
  let extension = file_path.extension()
    .and_then(|e| e.to_str())
    .unwrap_or("");
  
  // Check if any condition matches (simplified)
  let mut matches = false;
  for condition in &rule.conditions {
    match condition.r#type.as_str() {
      "extension" => {
        if let Some(target_ext) = condition.value.as_str() {
          if extension == target_ext {
            matches = true;
            break;
          }
        }
      },
      "nameMatches" => {
        if let Some(pattern) = condition.value.as_str() {
          if let Ok(regex) = regex::Regex::new(pattern) {
            if regex.is_match(file_name) {
              matches = true;
              break;
            }
          }
        }
      },
      _ => matches = true, // For simplicity, other conditions match
    }
  }
  
  if matches && !rule.actions.is_empty() {
    let action = &rule.actions[0];
    let destination = if let Some(target) = action.params.get("target") {
      target.as_str().unwrap_or("./Organized/")
        .replace("{extension}", extension)
        .replace("{filename}", file_name)
    } else {
      format!("./Organized/{}", file_name)
    };
      
    return Ok(Some(RulePreview {
      source_path: file_path.to_string_lossy().to_string(),
      destination_path: destination,
      action_type: action.r#type.clone(),
      confidence: 0.85, // Mock confidence score
    }));
  }
  
  Ok(None)
}
