// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct NeighborsSuccessOutputTopic {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_at: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_by: ::std::option::Option<::std::string::String>,
    pub id: NeighborsSuccessOutputStableId,
    pub links: ::std::vec::Vec<NeighborsSuccessOutputArtifactLink>,
    pub requirement_id: NeighborsSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: NeighborsSuccessOutputScopeId,
    pub status: NeighborsSuccessOutputTopicStatus,
    pub title: ::std::string::String,
}
