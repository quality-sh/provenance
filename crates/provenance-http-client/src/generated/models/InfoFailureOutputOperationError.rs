// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum InfoFailureOutputOperationError {
    #[serde(rename = "invalid_input")]
    InvalidInput {
        field: ::std::option::Option<::std::string::String>,
        reason: InfoFailureOutputInvalidInputReason,
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
    #[serde(rename = "file_access_denied")]
    FileAccessDenied,
    #[serde(rename = "file_unavailable")]
    FileUnavailable,
    #[serde(rename = "git_unavailable")]
    GitUnavailable,
    #[serde(rename = "git_revision_not_found")]
    GitRevisionNotFound,
    #[serde(rename = "no_projection")]
    NoProjection,
    #[serde(rename = "stale")]
    Stale {
        digest: ::std::string::String,
        instance_id: ::std::string::String,
        moved: ::std::vec::Vec<InfoFailureOutputMovedUnit>,
        serial: i64,
    },
    #[serde(rename = "unit_unreadable")]
    UnitUnreadable { unit: ::std::string::String },
    #[serde(rename = "schema_behind")]
    SchemaBehind,
    #[serde(rename = "half_migrated")]
    HalfMigrated,
    #[serde(rename = "read_failed")]
    ReadFailed,
}
