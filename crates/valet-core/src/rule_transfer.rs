//! Rule export and import (a Pro feature; the app checks the plan).
//!
//! File format: `{"format":"valet-rules","v":1,"rules":[Rule, ...]}`.
//! Only own rules are exported; presets are installed from the preset list.
//!
//! Every imported rule gets a new random id and `version = 1`, so no stored
//! approval (`approvals.json`, keyed by rule id and version) can match it:
//! auto-apply skips it until the user executes a preview that contains it.

use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::entitlements::PLAN_DISABLED;
use crate::rules::Rule;

pub const FORMAT: &str = "valet-rules";
pub const FORMAT_VERSION: u64 = 1;
/// Import files larger than this are refused before parsing.
pub const MAX_IMPORT_BYTES: usize = 1024 * 1024;
pub const MAX_IMPORT_RULES: usize = 500;

#[derive(Serialize)]
struct ExportFile<'a> {
    format: &'static str,
    v: u64,
    rules: Vec<&'a Rule>,
}

/// The export file for the own rules of `rules` (preset rules are left
/// out), in the given order, as pretty-printed JSON.
pub fn export_rules(rules: &[Rule]) -> String {
    let file = ExportFile {
        format: FORMAT,
        v: FORMAT_VERSION,
        rules: rules.iter().filter(|r| !r.is_preset()).collect(),
    };
    // A struct of strings, numbers and rules always serializes.
    serde_json::to_string_pretty(&file).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// More than [`MAX_IMPORT_BYTES`].
    TooLarge,
    /// Not JSON, or a rule that does not match the rule model.
    Malformed,
    /// `format` is not `valet-rules` or `v` is not 1.
    UnknownFormat,
    /// More than [`MAX_IMPORT_RULES`] rules.
    TooManyRules,
    /// Rule `index` (0-based) failed validation with `code` (`rule.*`).
    InvalidRule { index: usize, code: &'static str },
}

impl ImportError {
    pub fn code(&self) -> &'static str {
        match self {
            ImportError::TooLarge => "import.tooLarge",
            ImportError::Malformed => "import.malformed",
            ImportError::UnknownFormat => "import.unknownFormat",
            ImportError::TooManyRules => "import.tooManyRules",
            ImportError::InvalidRule { .. } => "import.invalidRule",
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::InvalidRule { index, code } => {
                write!(f, "{}: {} {code}", self.code(), index + 1)
            }
            _ => f.write_str(self.code()),
        }
    }
}

impl std::error::Error for ImportError {}

/// The rules of an export file, ready to store as new rules: each with a
/// new random id, `version = 1`, no `presetId` (an imported rule is an own
/// rule), and a `planDisabled` rule enabled again without the flag. Every
/// rule must pass `validate` (the app's `validate_rule`) as it will be
/// stored. All or nothing: the first problem fails the whole import.
pub fn import_rules(
    text: &str,
    validate: impl Fn(&Rule) -> Result<(), &'static str>,
) -> Result<Vec<Rule>, ImportError> {
    if text.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge);
    }
    let file: Value = serde_json::from_str(text).map_err(|_| ImportError::Malformed)?;
    if file.get("format").and_then(Value::as_str) != Some(FORMAT)
        || file.get("v").and_then(Value::as_u64) != Some(FORMAT_VERSION)
    {
        return Err(ImportError::UnknownFormat);
    }
    let Some(items) = file.get("rules").and_then(Value::as_array) else {
        return Err(ImportError::Malformed);
    };
    if items.len() > MAX_IMPORT_RULES {
        return Err(ImportError::TooManyRules);
    }
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let mut rule: Rule =
            serde_json::from_value(item.clone()).map_err(|_| ImportError::Malformed)?;
        rule.id = Uuid::new_v4();
        rule.version = 1;
        if let Some(o) = rule.options.as_object_mut() {
            o.remove("presetId");
            if o.remove(PLAN_DISABLED) == Some(Value::Bool(true)) {
                rule.enabled = true;
            }
        }
        validate(&rule).map_err(|code| ImportError::InvalidRule { index, code })?;
        out.push(rule);
    }
    Ok(out)
}
