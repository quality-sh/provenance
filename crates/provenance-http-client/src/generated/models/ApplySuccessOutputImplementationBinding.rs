// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct ApplySuccessOutputImplementationBinding {
    pub declared_by: ::std::string::String,
    pub file: ::std::string::String,
    pub id: ApplySuccessOutputStableId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub retired: ::std::option::Option<bool>,
    pub rule_id: ApplySuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: ApplySuccessOutputScopeId,
    pub symbol: ::std::string::String,
}
