// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanFailureOutputFinding {
    pub kind: PlanFailureOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: PlanFailureOutputRuleNumber,
    pub span: PlanFailureOutputSpan,
}
