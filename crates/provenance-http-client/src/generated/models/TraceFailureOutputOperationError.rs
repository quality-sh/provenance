// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum TraceFailureOutputOperationError {
    OperationFailure(TraceFailureOutputOperationFailure),
    ReadFailure(TraceFailureOutputReadFailure),
}
impl ::std::convert::From<TraceFailureOutputOperationFailure>
for TraceFailureOutputOperationError {
    fn from(value: TraceFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<TraceFailureOutputReadFailure>
for TraceFailureOutputOperationError {
    fn from(value: TraceFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
