use provenance_core::{
    ClaimChallenge, ConsensusFinding, ContestedClaim, ContributionStance, EvidenceGap,
    IdeationEvidenceReference, IdeationTarget, MaterialClaim, MinorityObjection,
    RequiredHumanDecision, ScopeId, StableId, SuggestedArtifact, SuggestedArtifactChange,
    UncertaintyRating, UnsupportedRecommendation, UnsupportedSpeculation,
};
use serde::Deserialize;

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateContributionInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub target: Option<IdeationTarget>,
    pub participant_slot: Option<String>,
    pub stance: Option<ContributionStance>,
    pub strongest_finding: Option<String>,
    pub evidence_references: Option<Vec<IdeationEvidenceReference>>,
    pub material_claims: Option<Vec<MaterialClaim>>,
    pub risks: Option<Vec<String>>,
    pub objections: Option<Vec<String>>,
    pub challenges: Option<Vec<ClaimChallenge>>,
    pub suggested_artifact_changes: Option<Vec<SuggestedArtifactChange>>,
    pub unsupported_recommendations: Option<Vec<UnsupportedRecommendation>>,
    pub uncertainty: Option<UncertaintyRating>,
    pub open_questions: Option<Vec<String>>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateSynthesisPacketInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub target: Option<IdeationTarget>,
    pub summary: Option<String>,
    pub consensus: Option<Vec<ConsensusFinding>>,
    pub contested_claims: Option<Vec<ContestedClaim>>,
    pub minority_objections: Option<Vec<MinorityObjection>>,
    pub evidence_gaps: Option<Vec<EvidenceGap>>,
    pub unsupported_speculation: Option<Vec<UnsupportedSpeculation>>,
    pub open_questions: Option<Vec<String>>,
    pub suggested_artifacts: Option<Vec<SuggestedArtifact>>,
    pub required_human_decisions: Option<Vec<RequiredHumanDecision>>,
}
