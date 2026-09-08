// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CompleteVerificationFailureOutputFinding {
    pub kind: CompleteVerificationFailureOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: CompleteVerificationFailureOutputRuleNumber,
    pub span: CompleteVerificationFailureOutputSpan,
}
