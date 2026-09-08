// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SearchRequestInputRepositoryContext {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness: ::std::option::Option<SearchRequestInputFreshnessPolicy>,
    pub repository: ::std::string::String,
    pub scope: ::std::string::String,
}
