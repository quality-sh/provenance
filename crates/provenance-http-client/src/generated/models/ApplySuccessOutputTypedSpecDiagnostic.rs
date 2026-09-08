// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ApplySuccessOutputTypedSpecDiagnostic {
    pub address: ApplySuccessOutputDeclarationAddress,
    pub disposition: ApplySuccessOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: ApplySuccessOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: ApplySuccessOutputTypedResourceKind,
    pub rule: ApplySuccessOutputRuleNumber,
    pub span: ApplySuccessOutputSpan,
    pub standard: ApplySuccessOutputStandard,
}
