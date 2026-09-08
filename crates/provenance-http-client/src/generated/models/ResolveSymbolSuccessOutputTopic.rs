// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ResolveSymbolSuccessOutputTopic {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_at: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_by: ::std::option::Option<::std::string::String>,
    pub id: ResolveSymbolSuccessOutputStableId,
    pub links: ::std::vec::Vec<ResolveSymbolSuccessOutputArtifactLink>,
    pub requirement_id: ResolveSymbolSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: ResolveSymbolSuccessOutputScopeId,
    pub status: ResolveSymbolSuccessOutputTopicStatus,
    pub title: ::std::string::String,
}
