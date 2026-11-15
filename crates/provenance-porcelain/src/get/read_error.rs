//! The failures of a get and the `{error, meta}` object that each consumer returns.

use provenance_macros::rule;
use std::fmt::Display;

/// A failure from validation, identity resolution, or an injected operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadError {
    InvalidOptions,
    NotFound,
    AmbiguousIdentity,
    /// The review host refused one request field.
    InvalidInput {
        field: String,
        message: String,
    },
    /// The review host could not finish the request now; the same request can run again.
    HostBusy(String),
    Operation(String),
}

impl Display for ReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("unsupported read options"),
            Self::NotFound => formatter.write_str("record does not exist"),
            Self::AmbiguousIdentity => formatter.write_str("record ID is not unique"),
            Self::InvalidInput { message, .. }
            | Self::HostBusy(message)
            | Self::Operation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ReadError {}

impl ReadError {
    /// The `{error, meta}` object that each consumer returns for this failure.
    #[rule("rule_mcp_get_failure_matches_cli")]
    pub fn failure(&self) -> serde_json::Value {
        let mut error = match self {
            Self::InvalidOptions => {
                serde_json::json!({"kind": "invalid_input", "reason": "invalid_value"})
            }
            Self::NotFound => serde_json::json!({"kind": "resource_not_found"}),
            Self::AmbiguousIdentity => serde_json::json!({
                "kind": "invalid_input", "reason": "invalid_value", "field": "target"
            }),
            Self::InvalidInput { field, .. } => serde_json::json!({
                "kind": "invalid_input", "reason": "invalid_value", "field": field
            }),
            Self::HostBusy(_) => serde_json::json!({"kind": "host_busy"}),
            Self::Operation(_) => serde_json::json!({"kind": "operation_failed"}),
        };
        error["message"] = serde_json::Value::String(self.to_string());
        serde_json::json!({"error": error, "meta": {}})
    }
}
