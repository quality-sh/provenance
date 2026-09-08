// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequestInputTypedSpecInput {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub adopt_unowned: ::std::vec::Vec<ApplyRequestInputTypedAdoptionTarget>,
    pub declared_by: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub requirements: ::std::vec::Vec<ApplyRequestInputTypedRequirementInput>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub rules: ::std::vec::Vec<ApplyRequestInputTypedRuleInput>,
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub sources: ::std::vec::Vec<ApplyRequestInputTypedSourceInput>,
    pub spec: ::std::string::String,
}
