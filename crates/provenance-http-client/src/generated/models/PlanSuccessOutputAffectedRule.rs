// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct PlanSuccessOutputAffectedRule {
    pub evidence: PlanSuccessOutputRuleEvidence,
    pub id: PlanSuccessOutputStableId,
    pub implementations: ::std::vec::Vec<PlanSuccessOutputImplementationSite>,
    pub verifications: ::std::vec::Vec<PlanSuccessOutputVerificationSite>,
}
