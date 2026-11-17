//! The failure object of a repository command that a local caller runs.
//!
//! The CLI with `--format json` and the MCP tools give a refused command as
//! one failure object. A failure of a known type keeps its kind and its
//! fields. Each other failure is `operation_failed`, and its message names
//! each cause.

use provenance_core::protocol::failure::OperationFailure;
use provenance_core::protocol::read_failure::UnparsableStateFile;
use provenance_store::repository::OpenFailure;
use provenance_store::statement_analysis::StatementViolations;
use serde_json::{json, Value};
use std::error::Error;

/// The `error` member of the failure object of `error`.
pub fn failure(error: &(dyn Error + Send + Sync + 'static)) -> Value {
    if let Some(state) = UnparsableStateFile::find(error) {
        return serde_json::to_value(state.failure()).expect("a failure is JSON");
    }
    if let Some(violations) = find::<StatementViolations>(error) {
        return json!({
            "kind": "statement_invalid",
            "diagnostics": violations.diagnostics,
        });
    }
    let mut failure = match (find::<OperationFailure>(error), find::<OpenFailure>(error)) {
        (Some(failure), _) => serde_json::to_value(failure).expect("a failure is JSON"),
        (None, Some(OpenFailure::UnknownScope { .. })) => {
            serde_json::to_value(OperationFailure::UnknownScope).expect("a failure is JSON")
        }
        _ => json!({"kind": "operation_failed"}),
    };
    failure["message"] = json!(message(error));
    failure
}

/// The first error of type `T` in the cause chain of `error`.
fn find<'a, T: Error + 'static>(error: &'a (dyn Error + 'static)) -> Option<&'a T> {
    std::iter::successors(Some(error), |cause| cause.source())
        .find_map(|cause| cause.downcast_ref::<T>())
}

/// The text of `error` and of each cause after it, joined by `: `. A cause
/// whose text the message already holds is not given again.
fn message(error: &(dyn Error + 'static)) -> String {
    let mut message = error.to_string();
    for cause in std::iter::successors(error.source(), |cause| cause.source()) {
        let text = cause.to_string();
        if !message.contains(&text) {
            message = format!("{message}: {text}");
        }
    }
    message
}
