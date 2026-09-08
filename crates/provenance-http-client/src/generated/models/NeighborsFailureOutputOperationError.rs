// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum NeighborsFailureOutputOperationError {
    OperationFailure(NeighborsFailureOutputOperationFailure),
    ReadFailure(NeighborsFailureOutputReadFailure),
}
impl ::std::convert::From<NeighborsFailureOutputOperationFailure>
for NeighborsFailureOutputOperationError {
    fn from(value: NeighborsFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<NeighborsFailureOutputReadFailure>
for NeighborsFailureOutputOperationError {
    fn from(value: NeighborsFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
