// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ApplyFailureOutput {
    pub error: ApplyFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<ApplyFailureOutputOperation>,
    pub protocol_version: ApplyFailureOutputProtocolVersion,
}
