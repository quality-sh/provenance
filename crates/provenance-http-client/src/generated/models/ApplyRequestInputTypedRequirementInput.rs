// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequestInputTypedRequirementInput {
    ///Keys of requirements in the same document this one depends on.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub depends_on: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub description: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<::std::string::String>,
    pub key: ::std::string::String,
    ///The key of the requirement in the same document this one refines.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub refines: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub sources: ::std::vec::Vec<::std::string::String>,
    ///The canonical id of the resolution this requirement came out of.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub spawned_by: ::std::option::Option<::std::string::String>,
    pub statement: ::std::string::String,
    ///Keys of older requirements in the same document this one replaces.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub supersedes: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
}
