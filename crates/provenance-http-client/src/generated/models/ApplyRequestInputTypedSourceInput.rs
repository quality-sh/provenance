// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequestInputTypedSourceInput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub id: ::std::option::Option<::std::string::String>,
    pub key: ::std::string::String,
    ///Semantic validation accepts supported Source type names and the linear, github, and jira aliases.
    pub kind: ::std::string::String,
    pub name: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub reference: ::std::option::Option<::std::string::String>,
    ///Keys of older sources in the same document this one replaces.
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub supersedes: ::std::option::Option<::std::vec::Vec<::std::string::String>>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub url: ::std::option::Option<::std::string::String>,
}
