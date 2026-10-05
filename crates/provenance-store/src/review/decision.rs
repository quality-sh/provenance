//! The guarded decision cycle for every enrolled graph record kind.

mod resolution;
mod submission;
mod withdrawal;

use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::{IdeationTargetType, NodeType, StableId};

/// Refuses a review submission addressed through another record.
fn validate_submission_address(
    proposal: &provenance_core::ProposalCard,
    addressed: Option<(NodeType, &StableId)>,
) -> anyhow::Result<()> {
    let proposal_kind = NodeType::from(proposal.traceability.target.artifact_type);
    let proposal_id_value = &proposal.traceability.target.artifact_id;
    if addressed.is_none_or(|(kind, id)| kind == proposal_kind && id == proposal_id_value) {
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
