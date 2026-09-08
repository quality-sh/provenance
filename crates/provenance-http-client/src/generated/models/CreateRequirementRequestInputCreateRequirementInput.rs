// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CreateRequirementRequestInputCreateRequirementInput {
    pub depends_on: ::std::vec::Vec<CreateRequirementRequestInputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub domain_id: ::std::option::Option<CreateRequirementRequestInputStableId>,
    pub id: CreateRequirementRequestInputStableId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_message: ::std::option::Option<CreateRequirementRequestInputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub origin_thread: ::std::option::Option<CreateRequirementRequestInputStableId>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub refines: ::std::option::Option<CreateRequirementRequestInputStableId>,
    pub scope_id: CreateRequirementRequestInputScopeId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub spawned_by: ::std::option::Option<CreateRequirementRequestInputStableId>,
    pub statement: ::std::string::String,
    pub status: CreateRequirementRequestInputRequirementStatus,
    pub supersedes: ::std::vec::Vec<CreateRequirementRequestInputStableId>,
}
