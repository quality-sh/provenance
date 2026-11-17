//! One refusal shape for `--format json`.
//!
//! A refusal from the local host reaches the CLI as a typed error. In text
//! mode the error prints as before. In JSON mode the CLI prints one
//! `{error,meta}` object with no `Error: ` prefix and puts the recovery hint
//! in `error.hint`. An error whose text is one JSON object prints as that
//! object. Each other error gives `operation_failed` and its text in
//! `error.message`.

use provenance_core::protocol::failure::OperationFailure;
use provenance_core::protocol::read_failure::UnparsableStateFile;
use provenance_porcelain::action::ActionError;
use provenance_porcelain::get::ReadError;
use serde_json::{json, Map, Value};

pub use provenance_cli::refusal::HostRefusal;

/// The `{error,meta}` object of an error for a JSON caller.
pub fn json_refusal(error: &anyhow::Error) -> Value {
    if let Some(refusal) = error.downcast_ref::<HostRefusal>() {
        let mut failure = refusal.failure.clone();
        if let Some(hint) = &refusal.hint {
            if let Some(object) = failure.get_mut("error").and_then(Value::as_object_mut) {
                object.insert("hint".into(), json!(hint));
            }
        }
        return failure;
    }
    if let Some(failure) = error.downcast_ref::<OperationFailure>() {
        let mut object = match serde_json::to_value(failure) {
            Ok(Value::Object(object)) => object,
            _ => Map::new(),
        };
        object.insert("message".into(), json!(failure.to_string()));
        return json!({"error": object, "meta": {}});
    }
    if let Some(read) = error.downcast_ref::<ReadError>() {
        return read.failure();
    }
    if let Some(action) = error.downcast_ref::<ActionError>() {
        return envelope(action_error(action), &action.to_string());
    }
    if let Some(state) = UnparsableStateFile::find(error.as_ref()) {
        return json!({"error": state.failure(), "meta": {}});
    }
    let text = format!("{error:#}");
    // The statement refusal of the store is an error whose text is its JSON report.
    if let Ok(report @ Value::Object(_)) = serde_json::from_str::<Value>(&text) {
        return report;
    }
    let mut object = Map::new();
    object.insert("kind".into(), json!("operation_failed"));
    envelope(object, &text)
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
