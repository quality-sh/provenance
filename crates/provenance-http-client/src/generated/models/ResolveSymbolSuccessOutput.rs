// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ResolveSymbolSuccessOutput {
    pub file: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_cause: ::std::option::Option<ResolveSymbolSuccessOutputFreshnessCause>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_error: ::std::option::Option<::std::string::String>,
    pub has_more: bool,
    pub limit: u32,
    pub operation: ResolveSymbolSuccessOutputOperation,
    pub protocol_version: ResolveSymbolSuccessOutputProtocolVersion,
    pub rules: ::std::vec::Vec<ResolveSymbolSuccessOutputGraphNode>,
    pub stamp: ResolveSymbolSuccessOutputStamp,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub symbol: ::std::option::Option<::std::string::String>,
}
