// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum CreateRuleFailureOutputOperationFailure {
    #[serde(rename = "invalid_input")]
    InvalidInput {
        field: ::std::option::Option<::std::string::String>,
        reason: CreateRuleFailureOutputInvalidInputReason,
    },
    #[serde(rename = "protocol_mismatch")]
    ProtocolMismatch { requested: u32, supported: u32 },
    #[serde(rename = "unknown_operation")]
    UnknownOperation,
    #[serde(rename = "unauthenticated")]
    Unauthenticated,
    #[serde(rename = "access_denied")]
    AccessDenied,
    #[serde(rename = "unknown_target")]
    UnknownTarget,
    #[serde(rename = "unknown_scope")]
    UnknownScope,
    #[serde(rename = "unavailable_needs")]
    UnavailableNeeds,
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "uncertain_write")]
    UncertainWrite,
}
