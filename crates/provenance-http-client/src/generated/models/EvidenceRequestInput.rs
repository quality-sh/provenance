// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRequestInput {
    pub context: EvidenceRequestInputRepositoryContext,
    pub request: EvidenceRequestInputEvidenceQuery,
}
