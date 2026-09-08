// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum InfoFailureOutputOperationError {
    OperationFailure(InfoFailureOutputOperationFailure),
    ReadFailure(InfoFailureOutputReadFailure),
}
impl ::std::convert::From<InfoFailureOutputOperationFailure>
for InfoFailureOutputOperationError {
    fn from(value: InfoFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<InfoFailureOutputReadFailure>
for InfoFailureOutputOperationError {
    fn from(value: InfoFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
