//! One semantic validator for native reads and early network refusals.
use super::{
    failure::{InvalidInputReason, OperationFailure},
    GetQuery, NeighborsQuery, SearchQuery, TraceQuery,
};
use crate::StableId;

#[derive(Debug, thiserror::Error)]
#[error("{source}")]
pub struct QueryValidation {
    #[source]
    source: anyhow::Error,
    failure: OperationFailure,
}
impl QueryValidation {
    pub fn into_native(self) -> anyhow::Error {
        self.source
    }
    pub fn into_failure(self) -> OperationFailure {
        self.failure
    }
}
fn check<T>(field: &str, result: anyhow::Result<T>) -> Result<T, QueryValidation> {
    result.map_err(|source| QueryValidation {
        source,
        failure: OperationFailure::InvalidInput {
            field: Some(format!("request.{field}")),
            reason: InvalidInputReason::InvalidValue,
        },
    })
}
fn version(requested: Option<u32>) -> Result<(), QueryValidation> {
    super::ensure_protocol_version(requested).map_err(|source| QueryValidation {
        source,
        failure: OperationFailure::ProtocolMismatch {
            requested: requested.unwrap_or(super::SDK_PROTOCOL_VERSION),
            supported: super::SDK_PROTOCOL_VERSION,
        },
    })
}
fn id(value: &str) -> Result<(), QueryValidation> {
    check("id", StableId::new(value)).map(|_| ())
}
fn relations(values: &[String]) -> Result<(), QueryValidation> {
    for name in values {
        check(
            "relations",
            if crate::model::relations::is_relation_name(name) {
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "{}",
                    crate::model::relations::unknown_relation_refusal(name)
                ))
            },
        )?;
    }
    Ok(())
}
impl GetQuery {
    pub fn validate(&self) -> Result<(), QueryValidation> {
        version(self.protocol_version)?;
        id(&self.id)
    }
}
impl SearchQuery {
    pub fn validate(&self) -> Result<(), QueryValidation> {
        version(self.protocol_version)?;
        check("limit", super::ensure_limit(self.limit))?;
        check(
            "text",
            if self.text.trim().is_empty() {
                Err(anyhow::anyhow!("search text must not be empty"))
            } else {
                Ok(())
            },
        )
    }
}
impl NeighborsQuery {
    pub fn validate(&self) -> Result<(), QueryValidation> {
        version(self.protocol_version)?;
        check("limit", super::ensure_limit(self.limit))?;
        relations(&self.relations)?;
        id(&self.id)
    }
}
impl TraceQuery {
    pub fn validate(&self) -> Result<(), QueryValidation> {
        version(self.protocol_version)?;
        check("limit", super::ensure_limit(self.limit))?;
        check("max_depth", super::ensure_max_depth(self.max_depth))?;
        relations(&self.relations)?;
        id(&self.id)
    }
}
