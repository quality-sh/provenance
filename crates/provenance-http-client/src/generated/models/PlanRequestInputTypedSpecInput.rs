// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PlanRequestInputTypedSpecInput {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub adopt_unowned: ::std::vec::Vec<PlanRequestInputTypedAdoptionTarget>,
    pub declared_by: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub requirements: ::std::vec::Vec<PlanRequestInputTypedRequirementInput>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub rules: ::std::vec::Vec<PlanRequestInputTypedRuleInput>,
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub sources: ::std::vec::Vec<PlanRequestInputTypedSourceInput>,
    pub spec: ::std::string::String,
}
