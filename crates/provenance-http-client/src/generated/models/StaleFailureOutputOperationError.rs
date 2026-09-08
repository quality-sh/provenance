// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum StaleFailureOutputOperationError {
    OperationFailure(StaleFailureOutputOperationFailure),
    ReadFailure(StaleFailureOutputReadFailure),
}
impl ::std::convert::From<StaleFailureOutputOperationFailure>
for StaleFailureOutputOperationError {
    fn from(value: StaleFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<StaleFailureOutputReadFailure>
for StaleFailureOutputOperationError {
    fn from(value: StaleFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
