//! Keep native error sources while publishing a closed safe projection.
#[cfg(feature = "host")]
use crate::cache::CompletionTimedOut;
#[cfg(feature = "host")]
use crate::operations::files::FileAccessRefusal;
use crate::operations::reader::ReadRefusal;
#[cfg(feature = "host")]
use crate::stale::git::GitRefusal;
use crate::state_store::readers::StateLineInvalid;
use crate::write_error::{
    failure_cause, in_repository, native, SourceFailure, WriteError, WriteFailure,
};
use provenance_core::protocol::{
    failure::OperationFailure,
    read_failure::{MovedUnit, ReadFailure},
};
use provenance_macros::rule;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ReadError(pub anyhow::Error);
impl From<anyhow::Error> for ReadError {
    fn from(error: anyhow::Error) -> Self {
        Self(error)
    }
}
impl From<OperationFailure> for ReadError {
    fn from(error: OperationFailure) -> Self {
        Self(error.into())
    }
}
impl super::HandlerFailure for ReadError {
    fn failure_status(&self) -> u16 {
        self.status()
    }
    fn log_cause(&self) -> Option<String> {
        self.unclassified_cause()
    }
}
impl super::HandlerFailure for WriteError {
    fn failure_status(&self) -> u16 {
        self.status()
    }
    fn log_cause(&self) -> Option<String> {
        self.unclassified_cause()
    }
}
impl ReadError {
    /// The same error with the repository root, so that its cause gives each
    /// path relative to the repository.
    pub(super) fn in_repository(self, root: &camino::Utf8Path) -> Self {
        Self(in_repository(root.as_std_path(), self.0))
    }
    pub(super) fn status(&self) -> u16 {
        if let Some(error) = self.common() {
            return error.status_code();
        }
        self.safe().status_code()
    }
    /// A common refusal that a read handler raised itself, such as an input
    /// refusal; it keeps its own kind and status. A store refusal of a common
    /// kind reads as that common kind.
    fn common(&self) -> Option<OperationFailure> {
        let error = native(&self.0);
        if let Some(error) = error.downcast_ref::<OperationFailure>() {
            return Some(error.clone());
        }
        match &error.downcast_ref::<SourceFailure>()?.failure {
            WriteFailure::InvalidInput { field, reason } => Some(OperationFailure::InvalidInput {
                field: field.clone(),
                reason: reason.clone(),
            }),
            WriteFailure::AccessDenied => Some(OperationFailure::AccessDenied),
            _ => None,
        }
    }
    /// The closed failure for the native error. An error of no known kind
    /// reads as `ReadFailed`.
    fn safe(&self) -> ReadFailure {
        let error = native(&self.0);
        if let Some(error) = error.downcast_ref::<ReadFailure>() {
            return error.clone();
        }
        // Git, the working tree and the local cache close exist only with `host`.
        #[cfg(feature = "host")]
        if let Some(error) = error.downcast_ref::<GitRefusal>() {
            return git_failure(error);
        }
        #[cfg(feature = "host")]
        if let Some(error) = error.downcast_ref::<FileAccessRefusal>() {
            return file_failure(error).unwrap_or_else(|| self.read_failed());
        }
        #[cfg(feature = "host")]
        if let Some(error) = error.downcast_ref::<CompletionTimedOut>() {
            return completion_failure(error);
        }
        if let Some(error) = error.downcast_ref::<ReadRefusal>() {
            return refusal_failure(error);
        }
        if let Some(error) = error.downcast_ref::<SourceFailure>() {
            return store_refusal(&error.failure).unwrap_or_else(|| self.read_failed());
        }
        StateLineInvalid::find(error).map_or_else(|| self.read_failed(), state_line_failure)
    }
    fn read_failed(&self) -> ReadFailure {
        ReadFailure::ReadFailed {
            cause: failure_cause(&self.0),
        }
    }
    /// The native cause chain of a failure of no known kind, with absolute
    /// paths, for the host log. The wire answer gives the cause with each
    /// path relative to the repository.
    pub fn unclassified_cause(&self) -> Option<String> {
        (self.common().is_none() && matches!(self.safe(), ReadFailure::ReadFailed { .. }))
            .then(|| format!("{:#}", native(&self.0)))
    }
}

/// The shard below the state directory, the line and the parse message of a
/// state line that does not parse.
#[rule("rule_state_parse_refusal_names_the_line")]
fn state_line_failure(error: &StateLineInvalid) -> ReadFailure {
    ReadFailure::InvalidStateFile {
        file: error.file(),
        line: error.line,
        message: error.message.clone(),
    }
}

/// The read failure of the same name as a store refusal. A store refusal
/// that no read declares reads as `ReadFailed`.
const fn store_refusal(failure: &WriteFailure) -> Option<ReadFailure> {
    match failure {
        WriteFailure::ResourceNotFound => Some(ReadFailure::ResourceNotFound),
        _ => None,
    }
}

#[cfg(feature = "host")]
const fn git_failure(error: &GitRefusal) -> ReadFailure {
    match error {
        GitRefusal::Unavailable { .. } => ReadFailure::GitUnavailable,
        GitRefusal::RevisionNotFound { .. } => ReadFailure::GitRevisionNotFound,
    }
}

#[cfg(feature = "host")]
/// The failure of a file refusal that has a kind; a missing or unreadable
/// file has no kind of its own.
const fn file_failure(error: &FileAccessRefusal) -> Option<ReadFailure> {
    match error {
        FileAccessRefusal::Unavailable => Some(ReadFailure::FileUnavailable),
        FileAccessRefusal::Denied => Some(ReadFailure::FileAccessDenied),
        FileAccessRefusal::Missing | FileAccessRefusal::Read(_) => None,
    }
}

#[cfg(feature = "host")]
/// The stage of a cache close that missed its deadline, without the lock
/// path or the connection count.
#[rule("rule_cache_completion_names_its_stage_at_the_deadline")]
const fn completion_failure(error: &CompletionTimedOut) -> ReadFailure {
    ReadFailure::CacheCloseTimedOut {
        stage: error.stage.public(),
    }
}

fn refusal_failure(refusal: &ReadRefusal) -> ReadFailure {
    match refusal {
        ReadRefusal::NoProjection { .. } => ReadFailure::NoProjection,
        ReadRefusal::Stale {
            serial,
            digest,
            instance_id,
            moved,
            ..
        } => ReadFailure::Stale {
            serial: *serial,
            digest: digest.clone(),
            instance_id: instance_id.clone(),
            moved: moved.iter().map(moved_unit).collect(),
        },
        ReadRefusal::UnitUnreadable { unit, .. } => {
            ReadFailure::UnitUnreadable { unit: unit.clone() }
        }
        ReadRefusal::SchemaBehind { .. } => ReadFailure::SchemaBehind,
        ReadRefusal::HalfMigrated { .. } => ReadFailure::HalfMigrated,
    }
}

fn moved_unit(unit: &crate::operations::reader::MovedUnit) -> MovedUnit {
    MovedUnit {
        unit: unit.unit.clone(),
        stored: unit.stored.clone(),
        live: unit.live.clone(),
    }
}
impl serde::Serialize for ReadError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.common() {
            Some(error) => error.serialize(serializer),
            None => self.safe().serialize(serializer),
        }
    }
}
#[cfg(feature = "schema")]
impl schemars::JsonSchema for ReadError {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ReadFailure".into()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        ReadFailure::json_schema(generator)
    }
}

#[cfg(test)]
#[cfg(feature = "host")]
#[path = "failures_tests.rs"]
mod tests;
