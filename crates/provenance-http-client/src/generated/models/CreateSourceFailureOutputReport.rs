// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateSourceFailureOutputReport {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<CreateSourceFailureOutputFinding>,
    pub issue: CreateSourceFailureOutputStandardIssue,
    pub standard: CreateSourceFailureOutputStandard,
}
