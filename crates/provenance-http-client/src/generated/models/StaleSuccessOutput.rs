// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct StaleSuccessOutput {
    pub base: ::std::string::String,
    pub files_changed: u32,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_cause: ::std::option::Option<StaleSuccessOutputFreshnessCause>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_error: ::std::option::Option<::std::string::String>,
    pub has_more: bool,
    pub head: ::std::string::String,
    pub limit: u32,
    pub operation: StaleSuccessOutputOperation,
    pub protocol_version: StaleSuccessOutputProtocolVersion,
    pub sites: ::std::vec::Vec<StaleSuccessOutputEvidenceDiffSite>,
    pub stamp: StaleSuccessOutputStamp,
    pub summary: StaleSuccessOutputEvidenceDiffSummary,
}
