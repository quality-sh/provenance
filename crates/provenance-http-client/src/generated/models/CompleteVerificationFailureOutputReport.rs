// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CompleteVerificationFailureOutputReport {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<CompleteVerificationFailureOutputFinding>,
    pub issue: CompleteVerificationFailureOutputStandardIssue,
    pub standard: CompleteVerificationFailureOutputStandard,
}
