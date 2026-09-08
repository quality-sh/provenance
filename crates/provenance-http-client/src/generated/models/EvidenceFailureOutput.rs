// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct EvidenceFailureOutput {
    pub error: EvidenceFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<EvidenceFailureOutputOperation>,
    pub protocol_version: EvidenceFailureOutputProtocolVersion,
}
