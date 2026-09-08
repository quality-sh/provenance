// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct GetSuccessOutputRule {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub declaration_address: ::std::option::Option<GetSuccessOutputDeclarationAddress>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub declared_by: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    pub id: GetSuccessOutputStableId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<GetSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<GetSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub requirement_ids: ::std::vec::Vec<GetSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub resolution_ids: ::std::vec::Vec<GetSuccessOutputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub retired: ::std::option::Option<bool>,
    pub schema_version: u32,
    pub scope_id: GetSuccessOutputScopeId,
    pub severity: GetSuccessOutputRuleSeverity,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_document: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_section: ::std::option::Option<::std::string::String>,
    pub statement: ::std::string::String,
    pub status: GetSuccessOutputRuleStatus,
}
