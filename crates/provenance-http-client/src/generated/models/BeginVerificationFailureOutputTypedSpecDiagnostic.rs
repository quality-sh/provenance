// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct BeginVerificationFailureOutputTypedSpecDiagnostic {
    pub address: BeginVerificationFailureOutputDeclarationAddress,
    pub disposition: BeginVerificationFailureOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: BeginVerificationFailureOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: BeginVerificationFailureOutputTypedResourceKind,
    pub rule: BeginVerificationFailureOutputRuleNumber,
    pub span: BeginVerificationFailureOutputSpan,
    pub standard: BeginVerificationFailureOutputStandard,
}
