// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateSourceFailureOutput {
    pub error: CreateSourceFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<CreateSourceFailureOutputOperation>,
    pub protocol_version: CreateSourceFailureOutputProtocolVersion,
}
