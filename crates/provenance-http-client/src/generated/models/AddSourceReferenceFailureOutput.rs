// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct AddSourceReferenceFailureOutput {
    pub error: AddSourceReferenceFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<AddSourceReferenceFailureOutputOperation>,
    pub protocol_version: AddSourceReferenceFailureOutputProtocolVersion,
}
