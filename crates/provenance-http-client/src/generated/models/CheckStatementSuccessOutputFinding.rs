// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CheckStatementSuccessOutputFinding {
    pub kind: CheckStatementSuccessOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: CheckStatementSuccessOutputRuleNumber,
    pub span: CheckStatementSuccessOutputSpan,
}
