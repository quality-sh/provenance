// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CheckStatementFailureOutput {
    pub error: CheckStatementFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<CheckStatementFailureOutputOperation>,
    pub protocol_version: CheckStatementFailureOutputProtocolVersion,
}
