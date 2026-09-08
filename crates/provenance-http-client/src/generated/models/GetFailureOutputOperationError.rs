// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum GetFailureOutputOperationError {
    OperationFailure(GetFailureOutputOperationFailure),
    ReadFailure(GetFailureOutputReadFailure),
}
impl ::std::convert::From<GetFailureOutputOperationFailure>
for GetFailureOutputOperationError {
    fn from(value: GetFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<GetFailureOutputReadFailure>
for GetFailureOutputOperationError {
    fn from(value: GetFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
