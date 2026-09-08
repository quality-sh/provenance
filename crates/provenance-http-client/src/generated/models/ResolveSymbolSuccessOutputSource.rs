// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ResolveSymbolSuccessOutputSource {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub commit_pin: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub declaration_address: ::std::option::Option<
        ResolveSymbolSuccessOutputDeclarationAddress,
    >,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub declared_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub effective_date: ::std::option::Option<i64>,
    pub id: ResolveSymbolSuccessOutputStableId,
    pub name: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<ResolveSymbolSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub reference: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub retired: ::std::option::Option<bool>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub review_date: ::std::option::Option<i64>,
    pub schema_version: u32,
    pub scope_id: ResolveSymbolSuccessOutputScopeId,
    pub source_type: ResolveSymbolSuccessOutputSourceType,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub supersedes: ::std::vec::Vec<ResolveSymbolSuccessOutputStableId>,
    pub url: ::std::option::Option<::std::string::String>,
}
