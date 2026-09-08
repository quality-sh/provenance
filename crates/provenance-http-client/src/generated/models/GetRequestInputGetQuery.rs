// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GetRequestInputGetQuery {
    pub id: ::std::string::String,
    #[serde(default)]
    pub include_retired: bool,
    pub node_type: GetRequestInputNodeType,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub protocol_version: ::std::option::Option<u32>,
}
