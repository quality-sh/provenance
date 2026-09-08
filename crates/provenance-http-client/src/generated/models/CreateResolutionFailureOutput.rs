// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateResolutionFailureOutput {
    pub error: CreateResolutionFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<CreateResolutionFailureOutputOperation>,
    pub protocol_version: CreateResolutionFailureOutputProtocolVersion,
}
