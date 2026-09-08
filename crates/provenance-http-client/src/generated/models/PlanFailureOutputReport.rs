// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanFailureOutputReport {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<PlanFailureOutputFinding>,
    pub issue: PlanFailureOutputStandardIssue,
    pub standard: PlanFailureOutputStandard,
}
