// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TraceRequestInput {
    pub context: TraceRequestInputRepositoryContext,
    pub request: TraceRequestInputTraceQuery,
}
