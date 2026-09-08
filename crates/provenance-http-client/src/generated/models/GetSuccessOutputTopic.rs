// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct GetSuccessOutputTopic {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_at: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_by: ::std::option::Option<::std::string::String>,
    pub id: GetSuccessOutputStableId,
    pub links: ::std::vec::Vec<GetSuccessOutputArtifactLink>,
    pub requirement_id: GetSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: GetSuccessOutputScopeId,
    pub status: GetSuccessOutputTopicStatus,
    pub title: ::std::string::String,
}
