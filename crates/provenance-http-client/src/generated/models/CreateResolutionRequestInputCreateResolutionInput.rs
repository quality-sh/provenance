// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CreateResolutionRequestInputCreateResolutionInput {
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
    pub id: CreateResolutionRequestInputStableId,
    pub inputs: ::std::vec::Vec<CreateResolutionRequestInputResolutionInput>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub made_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<CreateResolutionRequestInputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<CreateResolutionRequestInputStableId>,
    pub position: ::std::string::String,
    pub rationale: ::std::string::String,
    pub requirement_ids: ::std::vec::Vec<CreateResolutionRequestInputStableId>,
    pub scope_id: CreateResolutionRequestInputScopeId,
    pub status: CreateResolutionRequestInputResolutionStatus,
    pub supersedes: ::std::vec::Vec<CreateResolutionRequestInputStableId>,
    pub title: ::std::string::String,
}
