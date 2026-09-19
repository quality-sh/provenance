//! CLI-owned bindings for shared Porcelain capabilities.

use provenance_porcelain::{Action, Outcome, RecordRequest};
use serde::Serialize;
use std::fmt::{Display, Formatter};

const ACTION_NAMES: &[&str] = &["get", "check"];

/// Return the Porcelain action names exposed by the CLI binding.
pub const fn action_names() -> &'static [&'static str] {
    ACTION_NAMES
}

/// An explicit CLI output format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    /// Emit JSON result data.
    Json,
}

/// Render a shared outcome in the CLI-owned output shape.
pub fn render<T: Serialize>(
    outcome: &Outcome<T>,
    format: Option<OutputFormat>,
) -> serde_json::Result<String> {
    match format {
        None => Ok(outcome.summary.clone()),
        Some(OutputFormat::Json) => serde_json::to_string_pretty(&outcome.data),
    }
}

/// An invalid CLI Porcelain binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingError;

impl Display for BindingError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("use <target> get")
    }
}

impl std::error::Error for BindingError {}

/// Translate target-first CLI words into one semantic record request.
pub fn parse_record(words: &[&str]) -> Result<RecordRequest, BindingError> {
    match words {
        [target, "get"] if *target != "get" => Ok(RecordRequest::new(*target, Action::Get)),
        [target] if !target.is_empty() => Ok(RecordRequest::new(*target, Action::Get)),
        _ => Err(BindingError),
    }
}
