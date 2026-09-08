// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRuleFailureOutput {
    pub error: CreateRuleFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<CreateRuleFailureOutputOperation>,
    pub protocol_version: CreateRuleFailureOutputProtocolVersion,
}
