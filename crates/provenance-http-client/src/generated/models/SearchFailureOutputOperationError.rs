// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum SearchFailureOutputOperationError {
    OperationFailure(SearchFailureOutputOperationFailure),
    ReadFailure(SearchFailureOutputReadFailure),
}
impl ::std::convert::From<SearchFailureOutputOperationFailure>
for SearchFailureOutputOperationError {
    fn from(value: SearchFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<SearchFailureOutputReadFailure>
for SearchFailureOutputOperationError {
    fn from(value: SearchFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
