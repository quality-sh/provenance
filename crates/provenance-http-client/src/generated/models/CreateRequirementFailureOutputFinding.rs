// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRequirementFailureOutputFinding {
    pub kind: CreateRequirementFailureOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: CreateRequirementFailureOutputRuleNumber,
    pub span: CreateRequirementFailureOutputSpan,
}
