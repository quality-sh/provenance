// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct BeginVerificationFailureOutputFinding {
    pub kind: BeginVerificationFailureOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: BeginVerificationFailureOutputRuleNumber,
    pub span: BeginVerificationFailureOutputSpan,
}
