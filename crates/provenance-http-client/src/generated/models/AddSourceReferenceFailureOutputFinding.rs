// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct AddSourceReferenceFailureOutputFinding {
    pub kind: AddSourceReferenceFailureOutputFindingKind,
    pub message: ::std::string::String,
    pub rule: AddSourceReferenceFailureOutputRuleNumber,
    pub span: AddSourceReferenceFailureOutputSpan,
}
