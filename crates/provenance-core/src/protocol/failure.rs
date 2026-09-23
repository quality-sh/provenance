//! Safe failure data shared by operation adapters.

use super::ResponseMeta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum InvalidInputReason {
    Required,
    InvalidValue,
    MalformedJson,
    UnknownField,
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationFailure {
    #[error("invalid operation input")]
    InvalidInput {
        field: Option<String>,
        reason: InvalidInputReason,
    },
    #[error("request names protocol version {requested}; this engine speaks {supported}")]
    ProtocolMismatch { requested: u32, supported: u32 },
    #[error("unknown operation")]
    UnknownOperation,
    #[error("method not allowed for this route")]
    MethodNotAllowed,
    #[error("listener authentication required")]
    Unauthenticated,
    #[error("access denied")]
    AccessDenied,
    #[error("unknown repository target")]
    UnknownTarget,
    #[error("unknown scope")]
    UnknownScope,
    #[error("required execution resources are unavailable")]
    UnavailableNeeds,
    #[error("internal operation failure")]
    Internal,
}

impl OperationFailure {
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::InvalidInput { .. } | Self::ProtocolMismatch { .. } => 400,
            Self::UnknownOperation | Self::UnknownTarget | Self::UnknownScope => 404,
            Self::MethodNotAllowed => 405,
            Self::Unauthenticated => 401,
            Self::AccessDenied => 403,
            Self::UnavailableNeeds => 503,
            Self::Internal => 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[error("{error}")]
pub struct FailureEnvelope<E = OperationFailure> {
    pub error: E,
    pub meta: ResponseMeta,
}

impl FailureEnvelope {
    pub fn new(_: Option<&str>, error: OperationFailure) -> Self {
        Self {
            error,
            meta: ResponseMeta::default(),
        }
    }
}

/// Keeps the handler's native error separate from preparation failure.
#[derive(Debug, Serialize, thiserror::Error)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum OperationError<F> {
    #[error(transparent)]
    Common(OperationFailure),
    #[error(transparent)]
    Handler(F),
}

/// Exact declared failure payload after dispatch, with private response status.
#[derive(Debug, Serialize, thiserror::Error)]
#[error("operation failed")]
pub struct ErasedFailure {
    pub error: serde_json::Value,
    pub meta: ResponseMeta,
    #[serde(skip)]
    status: u16,
}
impl ErasedFailure {
    pub fn new(_: Option<&str>, error: OperationFailure) -> Self {
        let status = error.status_code();
        Self {
            error: serde_json::to_value(error).expect("common failure is JSON"),
            meta: ResponseMeta::default(),
            status,
        }
    }
    pub fn declared<E: Serialize>(operation: &'static str, error: E, status: u16) -> Self {
        serde_json::to_value(error).map_or_else(
            |_| Self::new(Some(operation), OperationFailure::Internal),
            |error| Self {
                error,
                meta: ResponseMeta::default(),
                status,
            },
        )
    }
    pub const fn status_code(&self) -> u16 {
        self.status
    }
}
impl From<FailureEnvelope> for ErasedFailure {
    fn from(value: FailureEnvelope) -> Self {
        Self::new(None, value.error)
    }
}
