// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct SearchSuccessOutput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_cause: ::std::option::Option<SearchSuccessOutputFreshnessCause>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_error: ::std::option::Option<::std::string::String>,
    pub has_more: bool,
    pub limit: u32,
    pub nodes: ::std::vec::Vec<SearchSuccessOutputGraphNode>,
    pub operation: SearchSuccessOutputOperation,
    pub protocol_version: SearchSuccessOutputProtocolVersion,
    pub stamp: SearchSuccessOutputStamp,
}
