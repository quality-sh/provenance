// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct InfoFailureOutput {
    pub error: InfoFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<InfoFailureOutputOperation>,
    pub protocol_version: InfoFailureOutputProtocolVersion,
}
