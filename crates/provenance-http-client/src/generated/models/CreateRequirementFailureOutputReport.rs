// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRequirementFailureOutputReport {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<CreateRequirementFailureOutputFinding>,
    pub issue: CreateRequirementFailureOutputStandardIssue,
    pub standard: CreateRequirementFailureOutputStandard,
}
