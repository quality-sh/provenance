// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ApplyFailureOutputReport {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<ApplyFailureOutputFinding>,
    pub issue: ApplyFailureOutputStandardIssue,
    pub standard: ApplyFailureOutputStandard,
}
