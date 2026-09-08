// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum CheckStatementFailureOutputOperationError {
    OperationFailure(CheckStatementFailureOutputOperationFailure),
    StatementFailure(CheckStatementFailureOutputStatementFailure),
}
impl ::std::convert::From<CheckStatementFailureOutputOperationFailure>
for CheckStatementFailureOutputOperationError {
    fn from(value: CheckStatementFailureOutputOperationFailure) -> Self {
        Self::OperationFailure(value)
    }
}
impl ::std::convert::From<CheckStatementFailureOutputStatementFailure>
for CheckStatementFailureOutputOperationError {
    fn from(value: CheckStatementFailureOutputStatementFailure) -> Self {
        Self::StatementFailure(value)
    }
}
