// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum CreateSourceFailureOutputOperationError {
    #[serde(rename = "invalid_input")]
    InvalidInput {
        field: ::std::option::Option<::std::string::String>,
        reason: CreateSourceFailureOutputInvalidInputReason,
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
    #[serde(rename = "schema_version")]
    SchemaVersion,
    #[serde(rename = "already_exists")]
    AlreadyExists,
    #[serde(rename = "invalid_commit_pin")]
    InvalidCommitPin,
    #[serde(rename = "scope_mismatch")]
    ScopeMismatch,
    #[serde(rename = "statement_invalid")]
    StatementInvalid { report: CreateSourceFailureOutputReport },
    #[serde(rename = "invalid_declaration")]
    InvalidDeclaration,
    #[serde(rename = "ownership_conflict")]
    OwnershipConflict {
        conflicts: ::std::vec::Vec<CreateSourceFailureOutputReconciledResource>,
    },
    #[serde(rename = "missing_reference")]
    MissingReference,
    #[serde(rename = "statement_rejected")]
    StatementRejected {
        diagnostics: ::std::vec::Vec<CreateSourceFailureOutputTypedSpecDiagnostic>,
    },
    #[serde(rename = "invalid_verification_target")]
    InvalidVerificationTarget,
    #[serde(rename = "retired_rule")]
    RetiredRule,
    #[serde(rename = "invalid_completion")]
    InvalidCompletion,
    #[serde(rename = "already_complete")]
    AlreadyComplete,
    #[serde(rename = "file_access_denied")]
    FileAccessDenied,
    #[serde(rename = "file_unavailable")]
    FileUnavailable,
    #[serde(rename = "write_failed")]
    WriteFailed,
}
