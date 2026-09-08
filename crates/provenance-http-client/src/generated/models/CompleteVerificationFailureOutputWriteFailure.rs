// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(tag = "kind")]
pub enum CompleteVerificationFailureOutputWriteFailure {
    #[serde(rename = "schema_version")]
    SchemaVersion,
    #[serde(rename = "already_exists")]
    AlreadyExists,
    #[serde(rename = "invalid_commit_pin")]
    InvalidCommitPin,
    #[serde(rename = "scope_mismatch")]
    ScopeMismatch,
    #[serde(rename = "statement_invalid")]
    StatementInvalid { report: CompleteVerificationFailureOutputReport },
    #[serde(rename = "invalid_declaration")]
    InvalidDeclaration,
    #[serde(rename = "ownership_conflict")]
    OwnershipConflict {
        conflicts: ::std::vec::Vec<CompleteVerificationFailureOutputReconciledResource>,
    },
    #[serde(rename = "missing_reference")]
    MissingReference,
    #[serde(rename = "statement_rejected")]
    StatementRejected {
        diagnostics: ::std::vec::Vec<
            CompleteVerificationFailureOutputTypedSpecDiagnostic,
        >,
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
    #[serde(rename = "uncertain_write")]
    UncertainWrite,
}
