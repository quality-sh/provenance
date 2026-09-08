// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanSuccessOutputTypedSpecDiagnostic {
    pub address: PlanSuccessOutputDeclarationAddress,
    pub disposition: PlanSuccessOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: PlanSuccessOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: PlanSuccessOutputTypedResourceKind,
    pub rule: PlanSuccessOutputRuleNumber,
    pub span: PlanSuccessOutputSpan,
    pub standard: PlanSuccessOutputStandard,
}
