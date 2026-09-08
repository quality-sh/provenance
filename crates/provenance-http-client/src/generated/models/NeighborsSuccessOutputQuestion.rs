// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct NeighborsSuccessOutputQuestion {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub answer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_at: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub contradicts: ::std::option::Option<NeighborsSuccessOutputStableId>,
    pub id: NeighborsSuccessOutputStableId,
    pub links: ::std::vec::Vec<NeighborsSuccessOutputArtifactLink>,
    pub question: ::std::string::String,
    pub requirement_id: NeighborsSuccessOutputStableId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub resolution_id: ::std::option::Option<NeighborsSuccessOutputStableId>,
    ///The verb that resolves this question, chosen when the question is minted.
    pub resolution_method: NeighborsSuccessOutputResolutionMethod,
    pub schema_version: u32,
    pub scope_id: NeighborsSuccessOutputScopeId,
    pub status: NeighborsSuccessOutputQuestionStatus,
    pub topic_id: NeighborsSuccessOutputStableId,
}
