// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CompleteVerificationSuccessOutput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub binding_id: ::std::option::Option<CompleteVerificationSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub commit: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub completed_at: ::std::option::Option<i64>,
    pub declared_by: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub error: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub file: ::std::option::Option<::std::string::String>,
    pub id: CompleteVerificationSuccessOutputStableId,
    pub method: ::std::string::String,
    pub rule_id: CompleteVerificationSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: CompleteVerificationSuccessOutputScopeId,
    pub started_at: i64,
    pub status: CompleteVerificationSuccessOutputVerificationRunStatus,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub symbol: ::std::option::Option<::std::string::String>,
}
