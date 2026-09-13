use crate::state_store::CreateProposalCardInput;
use provenance_core::{
    IdeationTarget, IdeationTargetType, PromotionState, ProposalTraceability, ProposalType,
    ScopeId, StableId,
};

mod disposition_allowlist;
mod disposition_references;
mod disposition_write_gate;
mod legacy_shard;
mod lifecycle_validation;
mod projection;

fn proposal_input(
    scope: &ScopeId,
    id: &str,
    title: &str,
    promotion_state: PromotionState,
) -> CreateProposalCardInput {
    CreateProposalCardInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        proposal_key: "overtime".into(),
        proposal_type: ProposalType::RequirementCandidate,
        title: title.into(),
        summary: "Clarify the overtime requirement.".into(),
        confidence: None,
        traceability: ProposalTraceability {
            target: IdeationTarget {
                artifact_type: IdeationTargetType::Requirement,
                artifact_id: StableId::new("req_overtime").unwrap(),
            },
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            supporting_claim_ids: Vec::new(),
        },
        builds_on: Vec::new(),
        promotion_state,
        duplicate_of: None,
        superseded_by: None,
        record_revision: None,
        revises: None,
        revises_rejection: None,
    }
}
