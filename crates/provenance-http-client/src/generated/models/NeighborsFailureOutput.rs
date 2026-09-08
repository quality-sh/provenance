// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct NeighborsFailureOutput {
    pub error: NeighborsFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<NeighborsFailureOutputOperation>,
    pub protocol_version: NeighborsFailureOutputProtocolVersion,
}
