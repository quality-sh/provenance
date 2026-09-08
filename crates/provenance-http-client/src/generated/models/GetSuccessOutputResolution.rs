// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct GetSuccessOutputResolution {
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
    pub id: GetSuccessOutputStableId,
    pub inputs: ::std::vec::Vec<GetSuccessOutputResolutionInput>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub made_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<GetSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<GetSuccessOutputStableId>,
    pub position: ::std::string::String,
    pub rationale: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub requirement_ids: ::std::vec::Vec<GetSuccessOutputStableId>,
    pub review_on: ::std::option::Option<::std::string::String>,
    pub schema_version: u32,
    pub scope_id: GetSuccessOutputScopeId,
    pub status: GetSuccessOutputResolutionStatus,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub supersedes: ::std::vec::Vec<GetSuccessOutputStableId>,
    pub title: ::std::string::String,
}
