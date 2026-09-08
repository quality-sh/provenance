// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct AddSourceReferenceFailureOutputReport {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<AddSourceReferenceFailureOutputFinding>,
    pub issue: AddSourceReferenceFailureOutputStandardIssue,
    pub standard: AddSourceReferenceFailureOutputStandard,
}
