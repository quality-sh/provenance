// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateResolutionFailureOutputReport {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<CreateResolutionFailureOutputFinding>,
    pub issue: CreateResolutionFailureOutputStandardIssue,
    pub standard: CreateResolutionFailureOutputStandard,
}
