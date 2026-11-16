//! Keep native error sources while publishing a closed safe projection.
#[cfg(feature = "host")]
use crate::cache::CompletionTimedOut;
#[cfg(feature = "host")]
use crate::operations::files::FileAccessRefusal;
use crate::operations::reader::{CatchUpFailed, ReadRefusal};
#[cfg(feature = "host")]
use crate::stale::git::GitRefusal;
use crate::write_error::{
    failure_cause, in_repository, native, SourceFailure, WriteError, WriteFailure,
};
use provenance_core::protocol::{
    failure::OperationFailure,
    read_failure::{MovedUnit, ReadFailure, UnparsableStateFile},
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
    fn message(&self) -> String {
        match (self.common(), self.safe()) {
            (None, ReadFailure::ReadFailed { cause }) => cause,
            _ => self.to_string(),
        }
    }
}
impl super::HandlerFailure for WriteError {
    fn failure_status(&self) -> u16 {
        self.status()
    }
    fn log_cause(&self) -> Option<String> {
        self.unclassified_cause()
    }
    fn message(&self) -> String {
        self.unclassified_message()
            .unwrap_or_else(|| self.to_string())
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
            WriteFailure::AccessDenied { message } => Some(OperationFailure::AccessDenied {
                message: message.clone(),
            }),
            _ => None,
        }
    }
    /// The closed failure for the native error. An error of no known kind
    /// reads as `ReadFailed`. Only the document read refuses a failed
    /// catch-up, so that refusal is `DocumentCatchUpFailed`.
    fn safe(&self) -> ReadFailure {
        let error = native(&self.0);
        let catch_up = error.downcast_ref::<CatchUpFailed>();
        catch_up.map_or_else(
            || self.classified(error, UnparsableStateFile::failure),
            |failed| self.catch_up_refusal(&failed.0),
        )
    }
    /// The refusal of a document read whose catch-up failed with `cause`: the
    /// location, the message and the hint of a state line that does not
    /// parse, else the cause as text.
    #[rule("rule_document_catch_up_refusal_gives_typed_cause")]
    fn catch_up_refusal(&self, cause: &anyhow::Error) -> ReadFailure {
        let message = match self.classified(cause, UnparsableStateFile::catch_up_failure) {
            failure @ ReadFailure::DocumentCatchUpFailed { .. } => return failure,
            ReadFailure::ReadFailed { cause } => cause,
            failure => failure.to_string(),
        };
        ReadFailure::DocumentCatchUpFailed {
            location: None,
            message,
            hint: None,
        }
    }
    /// The closed failure for `error`, the native error or a cause in it. A
    /// state line that does not parse gives the failure of `state_line`.
    fn classified(
        &self,
        error: &anyhow::Error,
        state_line: fn(&UnparsableStateFile) -> ReadFailure,
    ) -> ReadFailure {
        if let Some(error) = error.downcast_ref::<ReadFailure>() {
            return error.clone();
        }
        // Git, the working tree and the local cache close exist only with `host`.
        #[cfg(feature = "host")]
        if let Some(refusal) = error.downcast_ref::<GitRefusal>() {
            return git_failure(refusal);
        }
        #[cfg(feature = "host")]
        if let Some(refusal) = error.downcast_ref::<FileAccessRefusal>() {
            return file_failure(refusal).unwrap_or_else(|| self.read_failed(error));
        }
        #[cfg(feature = "host")]
        if let Some(timed_out) = error.downcast_ref::<CompletionTimedOut>() {
            return completion_failure(timed_out);
        }
        if let Some(refusal) = error.downcast_ref::<ReadRefusal>() {
            return refusal_failure(refusal);
        }
        if let Some(refusal) = error.downcast_ref::<SourceFailure>() {
            return store_refusal(&refusal.failure).unwrap_or_else(|| self.read_failed(error));
        }
        UnparsableStateFile::find(error.as_ref())
            .map_or_else(|| self.read_failed(error), state_line)
    }
    /// The `ReadFailed` failure of `error`, the native error or a cause in it.
    fn read_failed(&self, error: &anyhow::Error) -> ReadFailure {
        ReadFailure::ReadFailed {
            cause: failure_cause(&self.0, error),
        }
    }
    /// The native error, or the error of a failed catch-up in it.
    fn cause(&self) -> &anyhow::Error {
        let error = native(&self.0);
        error
            .downcast_ref::<CatchUpFailed>()
            .map_or(error, |failed| &failed.0)
    }
    /// The native cause chain of a failure of no known kind, with absolute
    /// paths, for the host log. The wire answer gives the cause with each
    /// path relative to the repository.
    pub fn unclassified_cause(&self) -> Option<String> {
        let cause = self.cause();
        let unclassified = matches!(
            self.classified(cause, UnparsableStateFile::failure),
            ReadFailure::ReadFailed { .. }
        );
        (self.common().is_none() && unclassified).then(|| format!("{cause:#}"))
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
#[provenance_macros::rule("rule_cache_completion_names_its_stage_at_the_deadline")]
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
