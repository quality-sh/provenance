// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct TraceSuccessOutput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_cause: ::std::option::Option<TraceSuccessOutputFreshnessCause>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_error: ::std::option::Option<::std::string::String>,
    pub has_more: bool,
    pub id: ::std::string::String,
    pub limit: u32,
    pub max_depth: u32,
    pub nodes: ::std::vec::Vec<TraceSuccessOutputTracedNode>,
    pub operation: TraceSuccessOutputOperation,
    pub protocol_version: TraceSuccessOutputProtocolVersion,
    pub stamp: TraceSuccessOutputStamp,
}
