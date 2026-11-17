//! A refused host operation as the CLI reports it.
//!
//! Text mode and JSON mode print the failure object. A failure of no known
//! kind gives its cause in the object.

use provenance_core::protocol::failure::ErasedFailure;
use serde_json::Value;

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

impl HostRefusal {
    /// The refusal for one host failure, printed as its failure object.
    pub fn of(failure: &ErasedFailure) -> Self {
        Self {
            failure: serde_json::to_value(failure).unwrap_or_default(),
            hint: None,
            text_shows_failure: true,
        }
    }
}

/// A refusal that the CLI gives itself: input that the parser accepted but
/// that has no valid value, or a check whose findings refuse.
#[derive(Debug)]
pub struct CommandRefusal {
    text: String,
    /// The `error` member of the failure object.
    pub failure: Value,
    /// Whether text mode prints the refusal as a usage error with exit code 2.
    pub usage: bool,
}

impl CommandRefusal {
    /// A usage refusal: an option that the command does not take, or an
    /// input value that it cannot read.
    pub fn usage(text: impl std::fmt::Display) -> Self {
        let text = text.to_string();
        let failure = serde_json::json!({
            "kind": "invalid_input",
            "field": null,
            "reason": "invalid_value",
            "message": text.trim_end(),
        });
        Self {
            text,
            failure,
            usage: true,
        }
    }

    /// A required input that the caller did not give.
    pub fn missing(field: &str, text: &str) -> Self {
        let failure = serde_json::json!({
            "kind": "invalid_input",
            "field": field,
            "reason": "required",
            "message": text,
        });
        Self {
            text: text.to_owned(),
            failure,
            usage: false,
        }
    }

    /// A refusal with the text `text` and the failure object `failure`.
    pub const fn new(text: String, failure: Value) -> Self {
        Self {
            text,
            failure,
            usage: false,
        }
    }

    /// The same refusal with the failure object `failure`.
    #[must_use]
    pub fn with_failure(mut self, failure: Value) -> Self {
        self.failure = failure;
        self
    }

    /// The text of the refusal.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for CommandRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.text)
    }
}

impl std::error::Error for CommandRefusal {}
