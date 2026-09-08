// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum EvidenceFailureOutputOperationError {
    OperationFailure(EvidenceFailureOutputOperationFailure),
    ReadFailure(EvidenceFailureOutputReadFailure),
}
impl ::std::convert::From<EvidenceFailureOutputOperationFailure>
for EvidenceFailureOutputOperationError {
    fn from(value: EvidenceFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<EvidenceFailureOutputReadFailure>
for EvidenceFailureOutputOperationError {
    fn from(value: EvidenceFailureOutputReadFailure) -> Self {
        Self::ReadFailure(value)
    }
}
