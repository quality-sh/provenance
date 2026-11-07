use super::LegacyProposalPolicy;
use crate::model::ideation::legacy_audit;
use crate::model::{DispositionRecord, PromotionState, ProposalCard};
use std::collections::BTreeSet;

pub(super) fn validate_records(
    proposals: &[ProposalCard],
    dispositions: &[DispositionRecord],
    policy: LegacyProposalPolicy,
) -> anyhow::Result<()> {
    let terminals = proposals
        .iter()
        .filter(|proposal| proposal.promotion_state != PromotionState::Proposed)
        .collect::<Vec<_>>();
    if terminals.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        policy == LegacyProposalPolicy::ShippedV1,
        "terminal proposal rows are forbidden by the modern-only lifecycle policy"
    );
    anyhow::ensure!(
        legacy_audit::is_shipped_terminal_proposal_set(terminals),
        "terminal proposal rows do not match the frozen shipped-v1 fingerprint"
    );
    validate_disposition_audit(proposals, dispositions)
}

fn validate_disposition_audit(
    proposals: &[ProposalCard],
    dispositions: &[DispositionRecord],
) -> anyhow::Result<()> {
    let terminal_ids = proposals
        .iter()
        .filter(|proposal| proposal.promotion_state != PromotionState::Proposed)
        .map(|proposal| proposal.id.as_str())
        .collect::<BTreeSet<_>>();
    anyhow::ensure!(
        !dispositions
            .iter()
            .any(|disposition| terminal_ids.contains(disposition.proposal_id.as_str())),
        "a terminal proposal row takes no disposition"
    );
    Ok(())
}
