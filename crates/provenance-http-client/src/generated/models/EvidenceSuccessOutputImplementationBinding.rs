// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct EvidenceSuccessOutputImplementationBinding {
    pub declared_by: ::std::string::String,
    pub file: ::std::string::String,
    pub id: EvidenceSuccessOutputStableId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub retired: ::std::option::Option<bool>,
    pub rule_id: EvidenceSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: EvidenceSuccessOutputScopeId,
    pub symbol: ::std::string::String,
}
