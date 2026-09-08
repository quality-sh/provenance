// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct NeighborsSuccessOutputBoundary {
    pub id: NeighborsSuccessOutputStableId,
    pub requirement_id: NeighborsSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: NeighborsSuccessOutputScopeId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_ref: ::std::option::Option<NeighborsSuccessOutputSourceReference>,
    pub statement: ::std::string::String,
}
