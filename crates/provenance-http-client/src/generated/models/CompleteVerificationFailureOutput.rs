// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CompleteVerificationFailureOutput {
    pub error: CompleteVerificationFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<CompleteVerificationFailureOutputOperation>,
    pub protocol_version: CompleteVerificationFailureOutputProtocolVersion,
}
