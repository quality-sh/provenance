// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ImpactRequestInputImpactQuery {
    pub id: ::std::string::String,
    #[serde(default)]
    pub include_retired: bool,
    #[serde(default = "defaults::default_nzu64::<::std::num::NonZeroU32, 50>")]
    pub limit: ::std::num::NonZeroU32,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub node_type: ::std::option::Option<ImpactRequestInputNodeType>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub protocol_version: ::std::option::Option<u32>,
}
