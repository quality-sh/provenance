// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRequirementFailureOutputTypedSpecDiagnostic {
    pub address: CreateRequirementFailureOutputDeclarationAddress,
    pub disposition: CreateRequirementFailureOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: CreateRequirementFailureOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: CreateRequirementFailureOutputTypedResourceKind,
    pub rule: CreateRequirementFailureOutputRuleNumber,
    pub span: CreateRequirementFailureOutputSpan,
    pub standard: CreateRequirementFailureOutputStandard,
}
