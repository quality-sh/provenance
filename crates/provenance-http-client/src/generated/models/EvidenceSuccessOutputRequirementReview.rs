// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct EvidenceSuccessOutputRequirementReview {
    pub after: ::std::string::String,
    pub before: ::std::string::String,
    pub changed_at: i64,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub cleared_at: ::std::option::Option<i64>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub cleared_by_run: ::std::option::Option<EvidenceSuccessOutputStableId>,
    pub field: ::std::string::String,
    pub id: EvidenceSuccessOutputStableId,
    pub requirement_id: EvidenceSuccessOutputStableId,
    pub rule_id: EvidenceSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: EvidenceSuccessOutputScopeId,
}
