// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct TraceSuccessOutputTopic {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_at: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_by: ::std::option::Option<::std::string::String>,
    pub id: TraceSuccessOutputStableId,
    pub links: ::std::vec::Vec<TraceSuccessOutputArtifactLink>,
    pub requirement_id: TraceSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: TraceSuccessOutputScopeId,
    pub status: TraceSuccessOutputTopicStatus,
    pub title: ::std::string::String,
}
