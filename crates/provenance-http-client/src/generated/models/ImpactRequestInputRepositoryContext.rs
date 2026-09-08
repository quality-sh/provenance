// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ImpactRequestInputRepositoryContext {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness: ::std::option::Option<ImpactRequestInputFreshnessPolicy>,
    pub repository: ::std::string::String,
    pub scope: ::std::string::String,
}
