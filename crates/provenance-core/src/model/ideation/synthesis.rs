use serde::{Deserialize, Serialize};

use super::{
    EvidenceQuality, IdeationEvidenceType, IdeationTarget, ProposalType, SpeculationMarker,
};
use crate::model::ids::{SchemaVersion, ScopeId, StableId};

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsensusFinding {
    pub statement: String,
    #[serde(alias = "supportingParticipantSlots")]
    pub supporting_participant_slots: Vec<String>,
    #[serde(alias = "evidenceReferenceIds")]
    pub evidence_reference_ids: Vec<StableId>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContestedClaim {
    #[serde(alias = "claimId")]
    pub claim_id: StableId,
    pub statement: String,
    #[serde(alias = "supportingParticipantSlots")]
    pub supporting_participant_slots: Vec<String>,
    #[serde(alias = "opposingParticipantSlots")]
    pub opposing_participant_slots: Vec<String>,
    #[serde(alias = "evidenceQuality")]
    pub evidence_quality: EvidenceQuality,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinorityObjection {
    #[serde(alias = "participantSlot")]
    pub participant_slot: String,
    pub objection: String,
    #[serde(alias = "evidenceReferenceIds")]
    pub evidence_reference_ids: Vec<StableId>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceGap {
    pub question: String,
    #[serde(alias = "neededEvidenceType")]
    pub needed_evidence_type: IdeationEvidenceType,
    #[serde(alias = "blockingPromotion")]
    pub blocking_promotion: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnsupportedSpeculation {
    pub statement: String,
    #[serde(alias = "originatingParticipantSlots")]
    pub originating_participant_slots: Vec<String>,
    pub marker: SpeculationMarker,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuggestedArtifact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_id: Option<StableId>,
    #[serde(alias = "proposalKey")]
    pub proposal_key: String,
    #[serde(alias = "proposalType")]
    pub proposal_type: ProposalType,
    pub summary: String,
    #[serde(alias = "originParticipantSlots")]
    pub origin_participant_slots: Vec<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredHumanDecision {
    #[serde(alias = "decisionKey")]
    pub decision_key: StableId,
    pub prompt: String,
    #[serde(alias = "blocksPromotion")]
    pub blocks_promotion: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthesisPacket {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub target: IdeationTarget,
    pub summary: String,
    pub consensus: Vec<ConsensusFinding>,
    #[serde(alias = "contestedClaims")]
    pub contested_claims: Vec<ContestedClaim>,
    #[serde(alias = "minorityObjections")]
    pub minority_objections: Vec<MinorityObjection>,
    #[serde(alias = "evidenceGaps")]
    pub evidence_gaps: Vec<EvidenceGap>,
    #[serde(alias = "unsupportedSpeculation")]
    pub unsupported_speculation: Vec<UnsupportedSpeculation>,
    #[serde(alias = "openQuestions")]
    pub open_questions: Vec<String>,
    #[serde(alias = "suggestedArtifacts")]
    pub suggested_artifacts: Vec<SuggestedArtifact>,
    #[serde(alias = "requiredHumanDecisions")]
    pub required_human_decisions: Vec<RequiredHumanDecision>,
}
