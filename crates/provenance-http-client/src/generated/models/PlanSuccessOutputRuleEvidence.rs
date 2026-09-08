// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanSuccessOutputRuleEvidence {
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub reasons: ::std::vec::Vec<PlanSuccessOutputReviewReason>,
    pub review_required: bool,
}
