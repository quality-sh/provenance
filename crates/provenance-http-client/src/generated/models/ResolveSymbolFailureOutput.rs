// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ResolveSymbolFailureOutput {
    pub error: ResolveSymbolFailureOutputOperationError,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub operation: ::std::option::Option<ResolveSymbolFailureOutputOperation>,
    pub protocol_version: ResolveSymbolFailureOutputProtocolVersion,
}
