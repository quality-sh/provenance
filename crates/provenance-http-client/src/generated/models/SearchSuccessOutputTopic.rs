// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct SearchSuccessOutputTopic {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_at: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_by: ::std::option::Option<::std::string::String>,
    pub id: SearchSuccessOutputStableId,
    pub links: ::std::vec::Vec<SearchSuccessOutputArtifactLink>,
    pub requirement_id: SearchSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: SearchSuccessOutputScopeId,
    pub status: SearchSuccessOutputTopicStatus,
    pub title: ::std::string::String,
}
