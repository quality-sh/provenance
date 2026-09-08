// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct EvidenceSuccessOutputVerificationBinding {
    pub declared_by: ::std::string::String,
    pub file: ::std::string::String,
    pub id: EvidenceSuccessOutputStableId,
    pub key: ::std::string::String,
    pub method: EvidenceSuccessOutputVerificationMethod,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub retired: ::std::option::Option<bool>,
    pub rule_id: EvidenceSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: EvidenceSuccessOutputScopeId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub symbol: ::std::option::Option<::std::string::String>,
}
