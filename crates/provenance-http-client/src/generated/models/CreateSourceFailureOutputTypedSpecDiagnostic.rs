// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateSourceFailureOutputTypedSpecDiagnostic {
    pub address: CreateSourceFailureOutputDeclarationAddress,
    pub disposition: CreateSourceFailureOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: CreateSourceFailureOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: CreateSourceFailureOutputTypedResourceKind,
    pub rule: CreateSourceFailureOutputRuleNumber,
    pub span: CreateSourceFailureOutputSpan,
    pub standard: CreateSourceFailureOutputStandard,
}
