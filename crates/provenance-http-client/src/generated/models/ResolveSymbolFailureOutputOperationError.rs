// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum ResolveSymbolFailureOutputOperationError {
    OperationFailure(ResolveSymbolFailureOutputOperationFailure),
    ReadFailure(ResolveSymbolFailureOutputReadFailure),
}
impl ::std::convert::From<ResolveSymbolFailureOutputOperationFailure>
for ResolveSymbolFailureOutputOperationError {
    fn from(value: ResolveSymbolFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<ResolveSymbolFailureOutputReadFailure>
for ResolveSymbolFailureOutputOperationError {
    fn from(value: ResolveSymbolFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
