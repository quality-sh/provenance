// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "preserve the declared wire fields")]
pub struct EvidenceSuccessOutput {
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_cause: ::std::option::Option<EvidenceSuccessOutputFreshnessCause>,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub freshness_error: ::std::option::Option<::std::string::String>,
    pub has_more: bool,
    pub implementation_bindings: ::std::vec::Vec<
        EvidenceSuccessOutputImplementationBinding,
    >,
    ///The cut flag of each list; `has_more` is the OR of the four.
    pub implementation_bindings_has_more: bool,
    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub latest_verification_run: ::std::option::Option<
        EvidenceSuccessOutputVerificationRun,
    >,
    pub limit: u32,
    pub operation: EvidenceSuccessOutputOperation,
    pub protocol_version: EvidenceSuccessOutputProtocolVersion,
    pub review_required: bool,
    pub reviews: ::std::vec::Vec<EvidenceSuccessOutputRequirementReview>,
    pub reviews_has_more: bool,
    pub rule_id: ::std::string::String,
    pub stale: ::std::option::Option<EvidenceSuccessOutputStaleEvidence>,
    pub stamp: EvidenceSuccessOutputStamp,
    pub verification_bindings: ::std::vec::Vec<EvidenceSuccessOutputVerificationBinding>,
    pub verification_bindings_has_more: bool,
    pub verification_runs: ::std::vec::Vec<EvidenceSuccessOutputVerificationRun>,
    pub verification_runs_has_more: bool,
}
