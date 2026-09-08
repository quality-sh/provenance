// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ResolveSymbolSuccessOutputBoundary {
    pub id: ResolveSymbolSuccessOutputStableId,
    pub requirement_id: ResolveSymbolSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: ResolveSymbolSuccessOutputScopeId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_ref: ::std::option::Option<ResolveSymbolSuccessOutputSourceReference>,
    pub statement: ::std::string::String,
}
