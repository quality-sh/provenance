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
