//! Why a repository did not open, and the common failure of each reason.

use crate::layout::GraphNotInitialized;
use crate::settings::SettingsError;
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};

/// Why a repository did not open.
#[derive(Debug, thiserror::Error)]
pub enum OpenFailure {
    #[error("resolve repository `{path}`: {error}")]
    InvalidRoot {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("{}", GraphNotInitialized)]
    NotInitialized,
    #[error("unknown scope `{selected}`; declared scopes: {}", .declared.join(", "))]
    UnknownScope {
        selected: String,
        declared: Vec<String>,
    },
    #[error(transparent)]
    Settings(#[from] SettingsError),
    /// The initialized-graph gate did not complete: an I/O failure, a lock
    /// failure, a manifest that is not a file, or a manifest that does not parse.
    #[error("{0:#}")]
    Io(anyhow::Error),
}

impl From<OpenFailure> for OperationFailure {
    fn from(failure: OpenFailure) -> Self {
        match failure {
            OpenFailure::InvalidRoot { .. } => Self::InvalidInput {
                field: None,
                reason: InvalidInputReason::InvalidValue,
            },
            OpenFailure::UnknownScope { .. } => Self::UnknownScope,
            OpenFailure::NotInitialized | OpenFailure::Settings(_) | OpenFailure::Io(_) => {
                Self::Internal
            }
        }
    }
}
