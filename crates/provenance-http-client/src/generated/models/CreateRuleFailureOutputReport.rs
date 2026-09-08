// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CreateRuleFailureOutputReport {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<CreateRuleFailureOutputFinding>,
    pub issue: CreateRuleFailureOutputStandardIssue,
    pub standard: CreateRuleFailureOutputStandard,
}
