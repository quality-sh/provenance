// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct AddSourceReferenceFailureOutputTypedSpecDiagnostic {
    pub address: AddSourceReferenceFailureOutputDeclarationAddress,
    pub disposition: AddSourceReferenceFailureOutputFindingKind,
    pub field: ::std::string::String,
    pub issue: AddSourceReferenceFailureOutputStandardIssue,
    pub message: ::std::string::String,
    pub resource_kind: AddSourceReferenceFailureOutputTypedResourceKind,
    pub rule: AddSourceReferenceFailureOutputRuleNumber,
    pub span: AddSourceReferenceFailureOutputSpan,
    pub standard: AddSourceReferenceFailureOutputStandard,
}
