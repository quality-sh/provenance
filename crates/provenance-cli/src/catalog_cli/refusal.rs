//! One refusal shape for `--format json`.
//!
//! A refusal from the local host reaches the CLI as a typed error. In text
//! mode the error prints as before. In JSON mode the CLI prints one
//! `{error,meta}` object with no `Error: ` prefix and puts the recovery hint
//! in `error.hint`.

use provenance_core::protocol::failure::OperationFailure;
use provenance_porcelain::action::ActionError;
use provenance_porcelain::get::ReadError;
use serde_json::{json, Map, Value};

/// A refused host operation with the recovery hint the CLI adds.
#[derive(Debug)]
pub struct HostRefusal {
    /// The `{error,meta}` object the host returned.
    pub failure: Value,
    /// Text the CLI adds so the caller knows the next command.
    pub hint: Option<String>,
    /// Whether the text form prints the failure object before the hint.
    pub text_shows_failure: bool,
}

impl std::fmt::Display for HostRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.text_shows_failure {
            formatter.write_str(&serde_json::to_string(&self.failure).unwrap_or_default())?;
            if let Some(hint) = &self.hint {
                write!(formatter, "; {hint}")?;
            }
            return Ok(());
        }
        formatter.write_str(self.hint.as_deref().unwrap_or("operation failed"))
    }
}

impl std::error::Error for HostRefusal {}

/// The `{error,meta}` object for an error that a JSON caller can parse, or
/// `None` when the error is not a host refusal.
pub fn json_refusal(error: &anyhow::Error) -> Option<Value> {
    if let Some(refusal) = error.downcast_ref::<HostRefusal>() {
        let mut failure = refusal.failure.clone();
        if let Some(hint) = &refusal.hint {
            if let Some(object) = failure.get_mut("error").and_then(Value::as_object_mut) {
                object.insert("hint".into(), json!(hint));
            }
        }
        return Some(failure);
    }
    if let Some(failure) = error.downcast_ref::<OperationFailure>() {
        let mut object = match serde_json::to_value(failure) {
            Ok(Value::Object(object)) => object,
            _ => Map::new(),
        };
        object.insert("message".into(), json!(failure.to_string()));
        return Some(json!({"error": object, "meta": {}}));
    }
    if let Some(read) = error.downcast_ref::<ReadError>() {
        return Some(envelope(read_error(read), &read.to_string()));
    }
    if let Some(action) = error.downcast_ref::<ActionError>() {
        return Some(envelope(action_error(action), &action.to_string()));
    }
    None
}

/// Prints the refusal object as the only line on standard error, with no
/// `Error: ` prefix, so a JSON caller can parse standard error, and exits 1.
#[provenance_macros::rule("rule_cli_json_refusal_object")]
pub fn exit_with_json(refusal: &Value) -> ! {
    eprintln!("{refusal}");
    std::process::exit(1);
}

fn envelope(mut object: Map<String, Value>, message: &str) -> Value {
    object.insert("message".into(), json!(message));
    json!({"error": object, "meta": {}})
}

fn read_error(error: &ReadError) -> Map<String, Value> {
    let object = match error {
        ReadError::InvalidOptions => invalid_input(None),
        ReadError::NotFound => json!({"kind": "resource_not_found"}),
        ReadError::AmbiguousIdentity => invalid_input(Some("target")),
        ReadError::Operation(_) => json!({"kind": "operation_failed"}),
    };
    object.as_object().cloned().unwrap_or_default()
}

fn action_error(error: &ActionError) -> Map<String, Value> {
    let object = match error {
        ActionError::OperationDetail { detail, .. } => detail
            .get("error")
            .cloned()
            .unwrap_or_else(|| detail.clone()),
        ActionError::NotFound => json!({"kind": "resource_not_found"}),
        ActionError::AccessDenied => json!({"kind": "access_denied"}),
        ActionError::Operation(_) => json!({"kind": "operation_failed"}),
        ActionError::KindSelection => invalid_input(Some("type")),
        ActionError::AmbiguousIdentity => invalid_input(Some("target")),
        ActionError::InvalidOptions => invalid_input(None),
    };
    object.as_object().cloned().unwrap_or_default()
}

fn invalid_input(field: Option<&str>) -> Value {
    let mut object = json!({"kind": "invalid_input", "reason": "invalid_value"});
    if let (Some(object), Some(field)) = (object.as_object_mut(), field) {
        object.insert("field".into(), json!(field));
    }
    object
}
