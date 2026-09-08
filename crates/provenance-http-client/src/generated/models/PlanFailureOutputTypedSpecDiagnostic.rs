// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanFailureOutputTypedSpecDiagnostic {
    pub address: PlanFailureOutputDeclarationAddress,
    pub disposition: PlanFailureOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: PlanFailureOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: PlanFailureOutputTypedResourceKind,
    pub rule: PlanFailureOutputRuleNumber,
    pub span: PlanFailureOutputSpan,
    pub standard: PlanFailureOutputStandard,
}
