// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct SearchFailureOutput {
    pub error: SearchFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<SearchFailureOutputOperation>,
    pub protocol_version: SearchFailureOutputProtocolVersion,
}
