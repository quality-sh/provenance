//! Keep native error sources while publishing a closed safe projection.
use crate::operations::files::FileAccessRefusal;
use crate::operations::reader::ReadRefusal;
use crate::stale::git::GitRefusal;
use provenance_core::protocol::{
    failure::OperationFailure,
    read_failure::{MovedUnit, ReadFailure},
};

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
        match self.safe() {
            ReadFailure::ResourceNotFound => 404,
            ReadFailure::ReadFailed => 500,
            ReadFailure::FileAccessDenied => 403,
            ReadFailure::FileUnavailable | ReadFailure::GitUnavailable => 503,
            _ => 409,
        }
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
        self.0
            .downcast_ref::<ReadRefusal>()
            .map_or(ReadFailure::ReadFailed, refusal_failure)
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
        self.safe().serialize(serializer)
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
