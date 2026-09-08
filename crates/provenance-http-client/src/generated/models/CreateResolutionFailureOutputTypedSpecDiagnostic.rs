// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateResolutionFailureOutputTypedSpecDiagnostic {
    pub address: CreateResolutionFailureOutputDeclarationAddress,
    pub disposition: CreateResolutionFailureOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: CreateResolutionFailureOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: CreateResolutionFailureOutputTypedResourceKind,
    pub rule: CreateResolutionFailureOutputRuleNumber,
    pub span: CreateResolutionFailureOutputSpan,
    pub standard: CreateResolutionFailureOutputStandard,
}
