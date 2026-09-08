// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequestInputTypedRuleInput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub address: ::std::option::Option<ApplyRequestInputDeclarationAddress>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub implementation: ::std::option::Option<ApplyRequestInputTypedImplementationInput>,
    pub key: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub name: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub requirement: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub requirements: ::std::vec::Vec<::std::string::String>,
    ///Canonical ids of the resolutions that produced this rule.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub resolution_ids: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
    pub statement: ::std::string::String,
}
