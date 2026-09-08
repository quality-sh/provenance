// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRequirementFailureOutput {
    pub error: CreateRequirementFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<CreateRequirementFailureOutputOperation>,
    pub protocol_version: CreateRequirementFailureOutputProtocolVersion,
}
