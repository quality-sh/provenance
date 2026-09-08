// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CreateRuleRequestInputCreateRuleInput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    pub id: CreateRuleRequestInputStableId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<CreateRuleRequestInputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<CreateRuleRequestInputStableId>,
    pub requirement_ids: ::std::vec::Vec<CreateRuleRequestInputStableId>,
    pub resolution_ids: ::std::vec::Vec<CreateRuleRequestInputStableId>,
    pub scope_id: CreateRuleRequestInputScopeId,
    pub severity: CreateRuleRequestInputRuleSeverity,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_document: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_section: ::std::option::Option<::std::string::String>,
    pub statement: ::std::string::String,
    pub status: CreateRuleRequestInputRuleStatus,
}
