// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct SearchSuccessOutputBoundary {
    pub id: SearchSuccessOutputStableId,
    pub requirement_id: SearchSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: SearchSuccessOutputScopeId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_ref: ::std::option::Option<SearchSuccessOutputSourceReference>,
    pub statement: ::std::string::String,
}
