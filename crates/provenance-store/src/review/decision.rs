//! The guarded decision cycle for every enrolled graph record kind.

mod resolution;
mod submission;
mod withdrawal;

use super::decision_state::CycleFacts;
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::{IdeationTargetType, NodeType, StableId};

fn validate_submission_address(
    proposal: &provenance_core::ProposalCard,
    facts: &CycleFacts,
    proposal_id: &StableId,
    addressed: Option<(NodeType, &StableId)>,
) -> anyhow::Result<()> {
    let proposal_kind = NodeType::from(proposal.traceability.target.artifact_type);
    let proposal_id_value = &proposal.traceability.target.artifact_id;
    let (cycle_kind, cycle_id) = facts
        .submission_address(proposal_id)
        .map_err(|error| SourceFailure::wrap(WriteFailure::InvalidUpdate, error))?;
    let matches_cycle = cycle_kind == proposal_kind && cycle_id == proposal_id_value;
    let matches_address =
        addressed.is_none_or(|(kind, id)| kind == proposal_kind && id == proposal_id_value);
    if matches_cycle && matches_address {
        return Ok(());
    }
    Err(SourceFailure::wrap(
        WriteFailure::InvalidUpdate,
        anyhow::anyhow!("the review submission does not belong to the addressed record"),
    ))
}

const fn target_type(kind: NodeType) -> IdeationTargetType {
    match kind {
        NodeType::Source => IdeationTargetType::Source,
        NodeType::Requirement => IdeationTargetType::Requirement,
        NodeType::Resolution => IdeationTargetType::Resolution,
        NodeType::Rule => IdeationTargetType::Rule,
        NodeType::Domain => IdeationTargetType::Domain,
        NodeType::Boundary => IdeationTargetType::Boundary,
        NodeType::Topic => IdeationTargetType::Topic,
        NodeType::Question => IdeationTargetType::Question,
    }
}

fn feedback_request_id(request: &StableId) -> anyhow::Result<StableId> {
    StableId::new(format!("{}_feedback", request.as_str()))
}
