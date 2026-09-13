//! Source failures retain native diagnostics separately from the safe wire form.
use crate::state_store::{
    ReconciledResource, StatementWriteError, TypedSpecDiagnostic, TypedSpecWriteError,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WriteFailure {
    SchemaVersion,
    AlreadyExists,
    InvalidCommitPin,
    ScopeMismatch,
    EmptyMessageBody,
    UnsupportedThreadParent,
    StatementInvalid {
        report: provenance_ste100::Report,
    },
    InvalidDeclaration,
    InvalidUpdate,
    RecordOwnershipConflict,
    OwnershipConflict {
        conflicts: Vec<ReconciledResource>,
    },
    MissingReference,
    StatementRejected {
        diagnostics: Vec<TypedSpecDiagnostic>,
    },
    InvalidVerificationTarget,
    InvalidCompletion,
    AlreadyComplete,
    FileAccessDenied,
    FileUnavailable,
    WriteFailed,
    UncertainWrite,
}

#[derive(Debug)]
pub struct SourceFailure {
    pub failure: WriteFailure,
    pub source: anyhow::Error,
}
impl std::fmt::Display for SourceFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.source, f)
    }
}
impl std::error::Error for SourceFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}
impl SourceFailure {
    pub fn wrap(failure: WriteFailure, source: impl Into<anyhow::Error>) -> anyhow::Error {
        let source = source.into();
        if source.downcast_ref::<Self>().is_some()
            || source.downcast_ref::<PublicationStarted>().is_some()
        {
            return source;
        }
        Self { failure, source }.into()
    }
}

/// This marker takes precedence over any nested validation failure.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct PublicationStarted(pub anyhow::Error);
pub(crate) fn publication_started(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<PublicationStarted>().is_some() {
        error
    } else {
        PublicationStarted(error).into()
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct WriteError(pub anyhow::Error);
impl From<anyhow::Error> for WriteError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}
impl From<provenance_core::protocol::failure::OperationFailure> for WriteError {
    fn from(error: provenance_core::protocol::failure::OperationFailure) -> Self {
        Self(error.into())
    }
}
impl WriteError {
    pub fn safe(&self) -> WriteFailure {
        if self.0.downcast_ref::<PublicationStarted>().is_some() {
            return WriteFailure::UncertainWrite;
        }
        if let Some(error) = self.0.downcast_ref::<SourceFailure>() {
            return error.failure.clone();
        }
        if let Some(error) = self.0.downcast_ref::<StatementWriteError>() {
            return WriteFailure::StatementInvalid {
                report: error.report.clone(),
            };
        }
        if let Some(error) = self.0.downcast_ref::<TypedSpecWriteError>() {
            return WriteFailure::StatementRejected {
                diagnostics: error.diagnostics.clone(),
            };
        }
        if let Some(error) = self
            .0
            .downcast_ref::<crate::operations::files::FileAccessRefusal>()
        {
            return match error {
                crate::operations::files::FileAccessRefusal::Denied => {
                    WriteFailure::FileAccessDenied
                }
                _ => WriteFailure::FileUnavailable,
            };
        }
        WriteFailure::WriteFailed
    }
    pub fn status(&self) -> u16 {
        match self.safe() {
            WriteFailure::WriteFailed | WriteFailure::UncertainWrite => 500,
            WriteFailure::FileAccessDenied => 403,
            WriteFailure::FileUnavailable => 503,
            WriteFailure::RecordOwnershipConflict
            | WriteFailure::AlreadyExists
            | WriteFailure::OwnershipConflict { .. }
            | WriteFailure::AlreadyComplete => 409,
            _ => 400,
        }
    }
}
impl Serialize for WriteError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.safe().serialize(serializer)
    }
}
#[cfg(feature = "schema")]
impl schemars::JsonSchema for WriteError {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WriteFailure".into()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        WriteFailure::json_schema(generator)
    }
}

macro_rules! ensure {
    ($kind:ident, $condition:expr, $($message:tt)*) => {
        if !$condition { return Err($crate::write_error::SourceFailure::wrap(
            $crate::write_error::WriteFailure::$kind, anyhow::anyhow!($($message)*))); }
    };
}
pub(crate) use ensure;
