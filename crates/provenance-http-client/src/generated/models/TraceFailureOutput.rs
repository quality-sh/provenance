// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct TraceFailureOutput {
    pub error: TraceFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<TraceFailureOutputOperation>,
    pub protocol_version: TraceFailureOutputProtocolVersion,
}
