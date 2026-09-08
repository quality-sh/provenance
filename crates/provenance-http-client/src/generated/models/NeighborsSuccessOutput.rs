// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct NeighborsSuccessOutput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_cause: ::std::option::Option<NeighborsSuccessOutputFreshnessCause>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_error: ::std::option::Option<::std::string::String>,
    pub has_more: bool,
    pub id: ::std::string::String,
    pub limit: u32,
    pub neighbors: ::std::vec::Vec<NeighborsSuccessOutputNeighbor>,
    pub operation: NeighborsSuccessOutputOperation,
    pub protocol_version: NeighborsSuccessOutputProtocolVersion,
    pub stamp: NeighborsSuccessOutputStamp,
}
