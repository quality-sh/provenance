// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct EvidenceSuccessOutputStaleEvidence {
    pub base: ::std::string::String,
    pub head: ::std::string::String,
    pub sites: ::std::vec::Vec<EvidenceSuccessOutputEvidenceDiffSite>,
}
