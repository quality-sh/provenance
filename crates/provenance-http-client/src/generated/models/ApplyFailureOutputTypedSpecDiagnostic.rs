// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ApplyFailureOutputTypedSpecDiagnostic {
    pub address: ApplyFailureOutputDeclarationAddress,
    pub disposition: ApplyFailureOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: ApplyFailureOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: ApplyFailureOutputTypedResourceKind,
    pub rule: ApplyFailureOutputRuleNumber,
    pub span: ApplyFailureOutputSpan,
    pub standard: ApplyFailureOutputStandard,
}
