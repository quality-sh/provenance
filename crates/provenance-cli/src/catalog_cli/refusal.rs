//! One refusal shape for `--format json`.
//!
//! A refusal from the local host reaches the CLI as a typed error. In text
//! mode the error prints as before. In JSON mode the CLI prints one
//! `{error,meta}` object with no `Error: ` prefix and puts the recovery hint
//! in `error.hint`. An error of no type that the CLI knows gets the failure
//! object that the MCP tools also give.

use provenance_core::protocol::read_failure::ReadFailure;
use provenance_porcelain::action::ActionError;
use provenance_porcelain::get::ReadError;
use serde_json::{json, Map, Value};

pub use provenance_cli::refusal::{CommandRefusal, HostRefusal};

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
    if let Some(refusal) = error.downcast_ref::<CommandRefusal>() {
        return json!({"error": refusal.failure, "meta": {}});
    }
    if let Some(read) = error.downcast_ref::<ReadError>() {
        return read.failure();
    }
    if let Some(action) = error.downcast_ref::<ActionError>() {
        return envelope(action_error(action), &action.to_string());
    }
    if let Some(held @ ReadFailure::StateFileInUse { .. }) = error.downcast_ref::<ReadFailure>() {
        return json!({"error": held, "meta": {}});
    }
    json!({"error": provenance_transport::refusal::failure(error.as_ref()), "meta": {}})
}

/// A usage refusal of an input that the parser accepted. With `--format json` it
/// is an `invalid_input` object; in text mode it is a usage error with exit code 2.
pub fn usage(error: impl std::fmt::Display) -> anyhow::Error {
    CommandRefusal::usage(error).into()
}

/// Exits as the parser does for a usage refusal: the text after `error: `,
/// which ends with a newline, and exit code 2.
pub fn exit_with_usage(refusal: &CommandRefusal) -> ! {
    let mut text = refusal.text().to_owned();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    clap::Error::raw(clap::error::ErrorKind::InvalidValue, text).exit()
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
