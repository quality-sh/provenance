//! Keep native error sources while publishing a closed safe projection.
use crate::operations::reader::ReadRefusal;
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
            ReadFailure::ReadFailed => 500,
            ReadFailure::FileAccessDenied => 403,
            ReadFailure::FileUnavailable | ReadFailure::GitUnavailable => 503,
            _ => 409,
        }
    }
    fn safe(&self) -> ReadFailure {
        if let Some(error) = self.0.downcast_ref::<ReadFailure>() {
            return error.clone();
        }
        if let Some(error) = self.0.downcast_ref::<crate::stale::git::GitRefusal>() {
            return match error {
                crate::stale::git::GitRefusal::Unavailable { .. } => ReadFailure::GitUnavailable,
                crate::stale::git::GitRefusal::RevisionNotFound { .. } => {
                    ReadFailure::GitRevisionNotFound
                }
            };
        }
        if let Some(error) = self
            .0
            .downcast_ref::<crate::operations::files::FileAccessRefusal>()
        {
            return match error {
                crate::operations::files::FileAccessRefusal::Unavailable => {
                    ReadFailure::FileUnavailable
                }
                crate::operations::files::FileAccessRefusal::Denied => {
                    ReadFailure::FileAccessDenied
                }
                _ => ReadFailure::ReadFailed,
            };
        }
        match self.0.downcast_ref::<ReadRefusal>() {
            Some(ReadRefusal::NoProjection { .. }) => ReadFailure::NoProjection,
            Some(ReadRefusal::Stale {
                serial,
                digest,
                instance_id,
                moved,
                ..
            }) => ReadFailure::Stale {
                serial: *serial,
                digest: digest.clone(),
                instance_id: instance_id.clone(),
                moved: moved
                    .iter()
                    .map(|unit| MovedUnit {
                        unit: unit.unit.clone(),
                        stored: unit.stored.clone(),
                        live: unit.live.clone(),
                    })
                    .collect(),
            },
            Some(ReadRefusal::UnitUnreadable { unit, .. }) => {
                ReadFailure::UnitUnreadable { unit: unit.clone() }
            }
            Some(ReadRefusal::SchemaBehind { .. }) => ReadFailure::SchemaBehind,
            Some(ReadRefusal::HalfMigrated { .. }) => ReadFailure::HalfMigrated,
            None => ReadFailure::ReadFailed,
        }
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
