// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRuleFailureOutputTypedSpecDiagnostic {
    pub address: CreateRuleFailureOutputDeclarationAddress,
    pub disposition: CreateRuleFailureOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: CreateRuleFailureOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: CreateRuleFailureOutputTypedResourceKind,
    pub rule: CreateRuleFailureOutputRuleNumber,
    pub span: CreateRuleFailureOutputSpan,
    pub standard: CreateRuleFailureOutputStandard,
}
