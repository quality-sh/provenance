// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct GetSuccessOutput {
    pub found: bool,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_cause: ::std::option::Option<GetSuccessOutputFreshnessCause>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_error: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub node: ::std::option::Option<GetSuccessOutputGraphNode>,
    pub operation: GetSuccessOutputOperation,
    pub protocol_version: GetSuccessOutputProtocolVersion,
    pub stamp: GetSuccessOutputStamp,
}
