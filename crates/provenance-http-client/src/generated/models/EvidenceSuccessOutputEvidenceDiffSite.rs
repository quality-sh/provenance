// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct EvidenceSuccessOutputEvidenceDiffSite {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub end_line: ::std::option::Option<u32>,
    pub file_path: ::std::string::String,
    pub kind: EvidenceSuccessOutputEvidenceSiteKind,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub line: ::std::option::Option<u32>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub original_file_path: ::std::option::Option<::std::string::String>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub original_line: ::std::option::Option<u32>,
    pub state: EvidenceSuccessOutputEvidenceDiffState,
    pub subject_id: ::std::string::String,
}
