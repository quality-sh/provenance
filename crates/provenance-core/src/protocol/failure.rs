//! Safe failure data shared by operation adapters.

use super::SDK_PROTOCOL_VERSION;
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
    #[error("listener authentication required")]
    Unauthenticated,
    #[error("access denied")]
    AccessDenied,
    #[error("unknown repository target")]
    UnknownTarget,
    #[error("required execution resources are unavailable")]
    UnavailableNeeds,
    #[error("internal operation failure")]
    Internal,
}

impl OperationFailure {
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::InvalidInput { .. } | Self::ProtocolMismatch { .. } => 400,
            Self::UnknownOperation | Self::UnknownTarget => 404,
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
    pub protocol_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    pub error: E,
}

impl FailureEnvelope {
    pub fn new(operation: Option<&str>, error: OperationFailure) -> Self {
        Self {
            protocol_version: SDK_PROTOCOL_VERSION,
            operation: operation.map(str::to_owned),
            error,
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
