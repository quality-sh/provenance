// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateSourceFailureOutputFinding {
    pub kind: CreateSourceFailureOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: CreateSourceFailureOutputRuleNumber,
    pub span: CreateSourceFailureOutputSpan,
}
