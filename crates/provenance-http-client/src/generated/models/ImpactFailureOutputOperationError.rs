// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum ImpactFailureOutputOperationError {
    OperationFailure(ImpactFailureOutputOperationFailure),
    ReadFailure(ImpactFailureOutputReadFailure),
}
impl ::std::convert::From<ImpactFailureOutputOperationFailure>
for ImpactFailureOutputOperationError {
    fn from(value: ImpactFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<ImpactFailureOutputReadFailure>
for ImpactFailureOutputOperationError {
    fn from(value: ImpactFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
