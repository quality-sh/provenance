// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct GetSuccessOutputBoundary {
    pub id: GetSuccessOutputStableId,
    pub requirement_id: GetSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: GetSuccessOutputScopeId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_ref: ::std::option::Option<GetSuccessOutputSourceReference>,
    pub statement: ::std::string::String,
}
