// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum VerificationRunsFailureOutputOperationError {
    OperationFailure(VerificationRunsFailureOutputOperationFailure),
    ReadFailure(VerificationRunsFailureOutputReadFailure),
}
impl ::std::convert::From<VerificationRunsFailureOutputOperationFailure>
for VerificationRunsFailureOutputOperationError {
    fn from(value: VerificationRunsFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<VerificationRunsFailureOutputReadFailure>
for VerificationRunsFailureOutputOperationError {
    fn from(value: VerificationRunsFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
