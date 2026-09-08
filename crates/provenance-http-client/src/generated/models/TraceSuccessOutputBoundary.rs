// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct TraceSuccessOutputBoundary {
    pub id: TraceSuccessOutputStableId,
    pub requirement_id: TraceSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: TraceSuccessOutputScopeId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub source_ref: ::std::option::Option<TraceSuccessOutputSourceReference>,
    pub statement: ::std::string::String,
}
