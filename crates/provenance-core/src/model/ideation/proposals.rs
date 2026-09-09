use serde::{Deserialize, Serialize};

use super::{
    lifecycle::AssertionId, IdeationEvidenceReference, IdeationTarget, PromotionState, ProposalType,
};
use crate::model::ids::{SchemaVersion, ScopeId, StableId};
use crate::model::validation::deserialize_optional_confidence;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProposalTraceability {
    pub target: IdeationTarget,
    #[serde(alias = "sourceIds")]
    pub source_ids: Vec<StableId>,
    #[serde(alias = "evidenceReferences")]
    pub evidence_references: Vec<IdeationEvidenceReference>,
    #[serde(alias = "supportingClaimIds")]
    pub supporting_claim_ids: Vec<StableId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProposalCard {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    #[serde(alias = "proposalId")]
    pub id: StableId,
    #[serde(alias = "proposalKey")]
    pub proposal_key: String,
    #[serde(alias = "proposalType")]
    pub proposal_type: ProposalType,
    pub title: String,
    pub summary: String,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_confidence",
        skip_serializing_if = "Option::is_none"
    )]
    pub confidence: Option<f64>,
    pub traceability: ProposalTraceability,
    #[serde(alias = "promotionState")]
    pub promotion_state: PromotionState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub builds_on: Vec<AssertionId>,
    #[serde(
        default,
        alias = "duplicateOfProposalId",
        skip_serializing_if = "Option::is_none"
    )]
    pub duplicate_of: Option<StableId>,
    #[serde(
        default,
        alias = "supersededByProposalId",
        skip_serializing_if = "Option::is_none"
    )]
    pub superseded_by: Option<StableId>,
}

pub type Proposal = ProposalCard;
