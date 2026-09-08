// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum VerificationBindingsFailureOutputOperationError {
    OperationFailure(VerificationBindingsFailureOutputOperationFailure),
    ReadFailure(VerificationBindingsFailureOutputReadFailure),
}
impl ::std::convert::From<VerificationBindingsFailureOutputOperationFailure>
for VerificationBindingsFailureOutputOperationError {
    fn from(value: VerificationBindingsFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<VerificationBindingsFailureOutputReadFailure>
for VerificationBindingsFailureOutputOperationError {
    fn from(value: VerificationBindingsFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
