// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRuleFailureOutputFinding {
    pub kind: CreateRuleFailureOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: CreateRuleFailureOutputRuleNumber,
    pub span: CreateRuleFailureOutputSpan,
}
