// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ResolveSymbolSuccessOutputResolution {
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
    pub id: ResolveSymbolSuccessOutputStableId,
    pub inputs: ::std::vec::Vec<ResolveSymbolSuccessOutputResolutionInput>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub made_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    pub position: ::std::string::String,
    pub rationale: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub requirement_ids: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
    pub review_on: ::std::option::Option<::std::string::String>,
    pub schema_version: u32,
    pub scope_id: ResolveSymbolSuccessOutputScopeId,
    pub status: ResolveSymbolSuccessOutputResolutionStatus,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub supersedes: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
    pub title: ::std::string::String,
}
