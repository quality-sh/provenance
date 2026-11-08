//! Keep native error sources while publishing a closed safe projection.
use crate::cache::CompletionTimedOut;
use crate::operations::files::FileAccessRefusal;
use crate::operations::reader::ReadRefusal;
use crate::stale::git::GitRefusal;
use crate::state_store::readers::StateLineInvalid;
use crate::write_error::{failure_cause, in_repository, native};
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
        match self.safe() {
            ReadFailure::ResourceNotFound => 404,
            ReadFailure::ReadFailed { .. } | ReadFailure::InvalidStateFile { .. } => 500,
            ReadFailure::FileAccessDenied => 403,
            ReadFailure::FileUnavailable
            | ReadFailure::GitUnavailable
            | ReadFailure::CacheCloseTimedOut { .. } => 503,
            _ => 409,
        }
    }
    /// A common refusal that a read handler raised itself, such as an input
    /// refusal; it keeps its own kind and status.
    fn common(&self) -> Option<&OperationFailure> {
        native(&self.0).downcast_ref::<OperationFailure>()
    }
    /// The closed failure for the native error. An error of no known kind
    /// reads as `ReadFailed`.
    fn safe(&self) -> ReadFailure {
        let error = native(&self.0);
        if let Some(error) = error.downcast_ref::<ReadFailure>() {
            return error.clone();
        }
        if let Some(error) = error.downcast_ref::<GitRefusal>() {
            return git_failure(error);
        }
        if let Some(error) = error.downcast_ref::<FileAccessRefusal>() {
            return file_failure(error).unwrap_or_else(|| self.read_failed());
        }
        if let Some(error) = error.downcast_ref::<CompletionTimedOut>() {
            return completion_failure(error);
        }
        if let Some(error) = error.downcast_ref::<ReadRefusal>() {
            return refusal_failure(error);
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

const fn git_failure(error: &GitRefusal) -> ReadFailure {
    match error {
        GitRefusal::Unavailable { .. } => ReadFailure::GitUnavailable,
        GitRefusal::RevisionNotFound { .. } => ReadFailure::GitRevisionNotFound,
    }
}

/// The failure of a file refusal that has a kind; a missing or unreadable
/// file has no kind of its own.
const fn file_failure(error: &FileAccessRefusal) -> Option<ReadFailure> {
    match error {
        FileAccessRefusal::Unavailable => Some(ReadFailure::FileUnavailable),
        FileAccessRefusal::Denied => Some(ReadFailure::FileAccessDenied),
        FileAccessRefusal::Missing | FileAccessRefusal::Read(_) => None,
    }
}

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
#[path = "failures_tests.rs"]
mod tests;
