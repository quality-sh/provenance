// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanSuccessOutputImplementationBinding {
    pub declared_by: ::std::string::String,
    pub file: ::std::string::String,
    pub id: PlanSuccessOutputStableId,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub retired: ::std::option::Option<bool>,
    pub rule_id: PlanSuccessOutputStableId,
    pub schema_version: u32,
    pub scope_id: PlanSuccessOutputScopeId,
    pub symbol: ::std::string::String,
}
