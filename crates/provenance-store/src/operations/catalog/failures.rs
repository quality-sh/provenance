//! Keep native error sources while publishing a closed safe projection.
use crate::cache::CompletionTimedOut;
use crate::operations::files::FileAccessRefusal;
use crate::operations::reader::ReadRefusal;
use crate::stale::git::GitRefusal;
use crate::state_store::readers::StateLineInvalid;
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
    pub(super) fn status(&self) -> u16 {
        if let Some(error) = self.common() {
            return error.status_code();
        }
        match self.safe() {
            ReadFailure::ResourceNotFound => 404,
            ReadFailure::ReadFailed | ReadFailure::InvalidStateFile { .. } => 500,
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
        self.0.downcast_ref::<OperationFailure>()
    }
    /// The closed failure for the native error. An error of no known kind
    /// reads as `ReadFailed`.
    fn safe(&self) -> ReadFailure {
        if let Some(error) = self.0.downcast_ref::<ReadFailure>() {
            return error.clone();
        }
        if let Some(error) = self.0.downcast_ref::<GitRefusal>() {
            return git_failure(error);
        }
        if let Some(error) = self.0.downcast_ref::<FileAccessRefusal>() {
            return file_failure(error);
        }
        if let Some(error) = self.0.downcast_ref::<CompletionTimedOut>() {
            return completion_failure(error);
        }
        if let Some(error) = self.0.downcast_ref::<ReadRefusal>() {
            return refusal_failure(error);
        }
        StateLineInvalid::find(&self.0).map_or(ReadFailure::ReadFailed, state_line_failure)
    }
    /// The native cause chain of a failure of no known kind. The wire answer
    /// gives only `read_failed`: the chain can hold absolute paths and stored
    /// values, so only the operator sees it.
    pub fn unclassified_cause(&self) -> Option<String> {
        (self.common().is_none() && self.safe() == ReadFailure::ReadFailed)
            .then(|| format!("{:#}", self.0))
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

const fn file_failure(error: &FileAccessRefusal) -> ReadFailure {
    match error {
        FileAccessRefusal::Unavailable => ReadFailure::FileUnavailable,
        FileAccessRefusal::Denied => ReadFailure::FileAccessDenied,
        FileAccessRefusal::Missing | FileAccessRefusal::Read(_) => ReadFailure::ReadFailed,
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
