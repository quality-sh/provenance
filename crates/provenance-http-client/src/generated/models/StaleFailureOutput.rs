// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct StaleFailureOutput {
    pub error: StaleFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<StaleFailureOutputOperation>,
    pub protocol_version: StaleFailureOutputProtocolVersion,
}
