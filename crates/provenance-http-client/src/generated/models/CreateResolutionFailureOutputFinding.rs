// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateResolutionFailureOutputFinding {
    pub kind: CreateResolutionFailureOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: CreateResolutionFailureOutputRuleNumber,
    pub span: CreateResolutionFailureOutputSpan,
}
