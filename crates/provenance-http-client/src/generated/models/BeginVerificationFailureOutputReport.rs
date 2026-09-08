// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct BeginVerificationFailureOutputReport {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<BeginVerificationFailureOutputFinding>,
    pub issue: BeginVerificationFailureOutputStandardIssue,
    pub standard: BeginVerificationFailureOutputStandard,
}
