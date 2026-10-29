//! Tauri commands, grouped by area.

pub(crate) mod license;
pub(crate) mod operations;
pub(crate) mod platform;
pub(crate) mod presets;
pub(crate) mod rules;
pub(crate) mod settings;

use serde::Serialize;
use valet_core::entitlements::{LimitKind, PlanLimit, PLAN_LIMIT_CODE};

/// Error of a command that can refuse a change because of the plan: the
/// usual code string (`code` or `code: detail`), or for `plan.limit` the
/// object `{ code: "plan.limit", kind: "rules" | "folders", limit, detail }`
/// (`detail` is `"rules 3"`, so `parseCommandError` also gets it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(crate) enum CmdError {
    Code(String),
    Limit {
        code: &'static str,
        kind: LimitKind,
        limit: usize,
        detail: String,
    },
}

impl From<String> for CmdError {
    fn from(code: String) -> Self {
        CmdError::Code(code)
    }
}

impl From<&str> for CmdError {
    fn from(code: &str) -> Self {
        CmdError::Code(code.to_string())
    }
}

impl From<PlanLimit> for CmdError {
    fn from(e: PlanLimit) -> Self {
        let kind = match e.kind {
            LimitKind::Rules => "rules",
            LimitKind::Folders => "folders",
        };
        CmdError::Limit {
            code: PLAN_LIMIT_CODE,
            kind: e.kind,
            limit: e.limit,
            detail: format!("{kind} {}", e.limit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_error_json() {
        assert_eq!(
            serde_json::to_value(CmdError::from("rule.notFound")).unwrap(),
            serde_json::json!("rule.notFound")
        );
        let e = CmdError::from(PlanLimit {
            kind: LimitKind::Rules,
            limit: 3,
        });
        assert_eq!(
            serde_json::to_value(e).unwrap(),
            serde_json::json!({ "code": "plan.limit", "kind": "rules", "limit": 3, "detail": "rules 3" })
        );
    }
}
