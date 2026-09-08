// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct VerificationBindingsRequestInputVerificationListRequest {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub rule: ::std::option::Option<VerificationBindingsRequestInputStableId>,
}
impl ::std::default::Default
for VerificationBindingsRequestInputVerificationListRequest {
    fn default() -> Self {
        Self { rule: Default::default() }
    }
}
