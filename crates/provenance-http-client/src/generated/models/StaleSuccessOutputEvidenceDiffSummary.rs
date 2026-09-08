// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct StaleSuccessOutputEvidenceDiffSummary {
    pub gone: u32,
    pub moved: u32,
    pub total_sites: u32,
    pub touched: u32,
    pub untouched: u32,
}
