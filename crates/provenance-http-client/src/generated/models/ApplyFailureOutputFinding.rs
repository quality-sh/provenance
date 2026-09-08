// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ApplyFailureOutputFinding {
    pub kind: ApplyFailureOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: ApplyFailureOutputRuleNumber,
    pub span: ApplyFailureOutputSpan,
}
