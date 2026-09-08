// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ResolveSymbolSuccessOutputQuestion {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub answer: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_at: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub claimed_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub contradicts: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    pub id: ResolveSymbolSuccessOutputStableId,
    pub links: ::std::vec::Vec<ResolveSymbolSuccessOutputArtifactLink>,
    pub question: ::std::string::String,
    pub requirement_id: ResolveSymbolSuccessOutputStableId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub resolution_id: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    ///The verb that resolves this question, chosen when the question is minted.
    pub resolution_method: ResolveSymbolSuccessOutputResolutionMethod,
    pub schema_version: u32,
    pub scope_id: ResolveSymbolSuccessOutputScopeId,
    pub status: ResolveSymbolSuccessOutputQuestionStatus,
    pub topic_id: ResolveSymbolSuccessOutputStableId,
}
