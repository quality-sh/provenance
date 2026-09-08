// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ImpactFailureOutput {
    pub error: ImpactFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<ImpactFailureOutputOperation>,
    pub protocol_version: ImpactFailureOutputProtocolVersion,
}
