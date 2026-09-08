// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateResolutionSuccessOutput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub approved_at: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub approved_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub confidence: ::std::option::Option<f64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub context: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub enforcement: ::std::option::Option<::std::string::String>,
    pub id: CreateResolutionSuccessOutputStableId,
    pub inputs: ::std::vec::Vec<CreateResolutionSuccessOutputResolutionInput>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub made_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<CreateResolutionSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<CreateResolutionSuccessOutputStableId>,
    pub position: ::std::string::String,
    pub rationale: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub requirement_ids: ::std::vec::Vec<CreateResolutionSuccessOutputStableId>,
    pub review_on: ::std::option::Option<::std::string::String>,
    pub schema_version: u32,
    pub scope_id: CreateResolutionSuccessOutputScopeId,
    pub status: CreateResolutionSuccessOutputResolutionStatus,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub supersedes: ::std::vec::Vec<CreateResolutionSuccessOutputStableId>,
    pub title: ::std::string::String,
}
