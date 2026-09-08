// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CompleteVerificationFailureOutputTypedSpecDiagnostic {
    pub address: CompleteVerificationFailureOutputDeclarationAddress,
    pub disposition: CompleteVerificationFailureOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: CompleteVerificationFailureOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: CompleteVerificationFailureOutputTypedResourceKind,
    pub rule: CompleteVerificationFailureOutputRuleNumber,
    pub span: CompleteVerificationFailureOutputSpan,
    pub standard: CompleteVerificationFailureOutputStandard,
}
