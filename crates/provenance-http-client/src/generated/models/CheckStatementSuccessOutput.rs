// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct CheckStatementSuccessOutput {
    pub analyzer_version: ::std::string::String,
    pub findings: ::std::vec::Vec<CheckStatementSuccessOutputFinding>,
    pub issue: CheckStatementSuccessOutputStandardIssue,
    pub standard: CheckStatementSuccessOutputStandard,
}
