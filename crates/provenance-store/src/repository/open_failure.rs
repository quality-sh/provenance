//! Why a repository did not open, and the common failure of each reason.

use crate::layout::GraphNotInitialized;
use crate::settings::SettingsError;
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};
#[cfg(feature = "host")]
use provenance_core::protocol::read_failure::ReadFailure;

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
    /// The text and the causes are those of the error, so a caller that shows
    /// only the text of the failure shows the fault.
    #[error(transparent)]
    Io(anyhow::Error),
}

impl OpenFailure {
    /// The typed read refusal of an open that the recovery at the section
    /// entry stopped: `state_file_in_use` with the file below the
    /// repository, when a different process holds a state file that the
    /// recovery must replace. Any other failure has none.
    #[cfg(feature = "host")]
    pub fn read_refusal(&self) -> Option<ReadFailure> {
        let Self::Io(error) = self else {
            return None;
        };
        error
            .chain()
            .find_map(|cause| cause.downcast_ref::<crate::write_error::StateFileInUse>())
            .map(|held| ReadFailure::StateFileInUse {
                file: held.file.clone(),
            })
    }
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
