// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CompleteVerificationRequestInput {
    pub context: CompleteVerificationRequestInputRepositoryScope,
    pub request: CompleteVerificationRequestInputCompleteVerificationInput,
}
