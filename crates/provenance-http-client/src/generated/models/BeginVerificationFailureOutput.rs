// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct BeginVerificationFailureOutput {
    pub error: BeginVerificationFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<BeginVerificationFailureOutputOperation>,
    pub protocol_version: BeginVerificationFailureOutputProtocolVersion,
}
