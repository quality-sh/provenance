// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct VerificationBindingsSuccessOutputVerificationBinding {
    pub declared_by: ::std::string::String,
    pub file: ::std::string::String,
    pub id: VerificationBindingsSuccessOutputStableId,
    pub key: ::std::string::String,
    pub method: VerificationBindingsSuccessOutputVerificationMethod,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub retired: ::std::option::Option<bool>,
    pub rule_id: VerificationBindingsSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: VerificationBindingsSuccessOutputScopeId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub symbol: ::std::option::Option<::std::string::String>,
}
