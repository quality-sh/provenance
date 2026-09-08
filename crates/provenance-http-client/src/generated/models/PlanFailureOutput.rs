// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanFailureOutput {
    pub error: PlanFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<PlanFailureOutputOperation>,
    pub protocol_version: PlanFailureOutputProtocolVersion,
}
