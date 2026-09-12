//! Shared decision-cycle helpers: receipt replay, validated cycle facts, and
//! the checks a submission or a decision must pass against the scope.

use crate::state_store::StateStore;
use provenance_core::{
    review::{CycleEntry, CycleFact, JournalEntry},
    DispositionDecision, IdeationTargetType, ProposalType, ScopeId, StableId,
};

pub(super) fn request_digest(input: &impl serde::Serialize) -> anyhow::Result<String> {
    anyhow::ensure!(
        serde_json::to_vec(input)?.len() <= 1_048_576,
        "review request exceeds the operation byte budget"
    );
    Ok(crate::canonical_digest::digest(
        &crate::canonical_digest::canonical_bytes(input)?,
    ))
}

/// Returns the committed receipt for this request after identity and intent
/// checks. An absent receipt is authoritative: the caller holds the
/// publication lock, so recovery has already run.
pub(super) fn replay(
    store: &StateStore,
    scope: &ScopeId,
    request: &StableId,
    actor: &str,
    digest: &str,
) -> anyhow::Result<Option<CycleEntry>> {
    let path = super::journal::entry_path(&store.layout, scope, request);
    if !path.try_exists()? {
        return Ok(None);
    }
    let entry = match super::journal::read_journal_entry(&store.layout, &path)? {
        JournalEntry::Cycle(entry) => *entry,
        _ => anyhow::bail!("request ID belongs to another review write"),
    };
    anyhow::ensure!(
        entry.scope_id == *scope
            && entry.request_id == *request
            && entry.actor == actor
            && entry.intent_digest == digest,
        "review request ID was reused with different intent"
    );
    Ok(Some(entry))
}

pub(super) fn write_receipt(store: &StateStore, entry: &CycleEntry) -> anyhow::Result<()> {
    anyhow::ensure!(
        serde_json::to_vec(entry)?.len() as u64 <= super::journal::ENTRY_BYTES,
        "review receipt exceeds the entry byte budget"
    );
    super::journal::write_new(
        &super::journal::entry_path(&store.layout, &entry.scope_id, &entry.request_id),
        entry,
    )
}

/// The scope's decision-cycle receipts, each checked against the records it
/// names, so a forged or dangling receipt cannot join the decision history.
pub(super) fn validated_cycle_entries(
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<Vec<CycleEntry>> {
    let entries = store.cycle_entries(scope)?;
    let proposals = store.list_proposal_definitions(scope)?;
    let dispositions = store.list_dispositions(scope)?;
    let mut ids = std::collections::BTreeSet::new();
    let mut sequences = std::collections::BTreeSet::new();
    for entry in &entries {
        anyhow::ensure!(
            ids.insert(entry.id.as_str()),
            "duplicate cycle entry identity"
        );
        anyhow::ensure!(
            !entry.actor.trim().is_empty(),
            "invalid decision-cycle actor"
        );
        anyhow::ensure!(
            sequences.insert((entry.requirement_id.as_str(), entry.sequence)),
            "duplicate decision-cycle sequence {} for requirement {}",
            entry.sequence,
            entry.requirement_id.as_str()
        );
        anyhow::ensure!(
            proposals.iter().any(|p| p.id == entry.proposal_id),
            "cycle entry names proposal {} which does not exist",
            entry.proposal_id.as_str()
        );
        match (entry.fact, &entry.disposition_id) {
            (CycleFact::Decided, Some(disposition_id)) => {
                let recorded = dispositions
                    .iter()
                    .find(|d| d.id == *disposition_id)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "cycle entry names disposition {} which does not exist",
                            disposition_id.as_str()
                        )
                    })?;
                anyhow::ensure!(
                    recorded.proposal_id == entry.proposal_id,
                    "cycle entry disposition does not belong to its proposal"
                );
            }
            (CycleFact::Decided, None) => {
                anyhow::bail!("a decided cycle entry names no disposition")
            }
            (CycleFact::Submitted | CycleFact::Withdrawn, None) => {}
            (CycleFact::Submitted | CycleFact::Withdrawn, Some(_)) => {
                anyhow::bail!("a submitted or withdrawn cycle entry names no disposition")
            }
        }
        if entry.fact == CycleFact::Submitted {
            anyhow::ensure!(
                proposals.iter().any(|p| p.id == entry.proposal_id
                    && p.proposal_type == ProposalType::RecordRevision),
                "a submitted cycle entry names a proposal that is not a review submission"
            );
        }
    }
    Ok(entries)
}

/// The validated decision-cycle facts of one scope.
pub(super) struct CycleFacts {
    entries: Vec<CycleEntry>,
}

impl CycleFacts {
    pub(super) fn validated(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Self> {
        Ok(Self {
            entries: validated_cycle_entries(store, scope)?,
        })
    }

    pub(super) fn is_withdrawn(&self, proposal: &StableId) -> bool {
        self.entries
            .iter()
            .any(|e| e.fact == CycleFact::Withdrawn && e.proposal_id == *proposal)
    }

    pub(super) fn is_decided(&self, proposal: &StableId) -> bool {
        self.entries
            .iter()
            .any(|e| e.fact == CycleFact::Decided && e.proposal_id == *proposal)
    }

    /// The feedback Message a decision published, if it did.
    pub(super) fn feedback_for(&self, disposition: &StableId) -> Option<StableId> {
        self.entries.iter().find_map(|e| {
            if e.fact == CycleFact::Decided && e.disposition_id.as_ref() == Some(disposition) {
                e.feedback_message_id.clone()
            } else {
                None
            }
        })
    }

    /// The submission sequence of one proposal, ordering its decision in the
    /// history. Receipts without a submission keep the end of the order.
    pub(super) fn sequence_of(&self, proposal: &StableId) -> Option<u64> {
        self.entries
            .iter()
            .filter(|e| e.fact == CycleFact::Submitted && e.proposal_id == *proposal)
            .map(|e| e.sequence)
            .next()
    }

    /// The withdrawn submissions of one record, in withdrawal order.
    pub(super) fn withdrawn_submissions(&self, requirement: &StableId) -> Vec<StableId> {
        let mut withdrawn: Vec<(u64, StableId)> = self
            .entries
            .iter()
            .filter(|e| e.requirement_id == *requirement && e.fact == CycleFact::Withdrawn)
            .map(|e| (e.sequence, e.proposal_id.clone()))
            .collect();
        withdrawn.sort_by_key(|(sequence, _)| *sequence);
        withdrawn
            .into_iter()
            .map(|(_, proposal)| proposal)
            .collect()
    }

    pub(super) fn next_sequence(&self, requirement: &StableId) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.requirement_id == *requirement)
            .map(|e| e.sequence)
            .max()
            .unwrap_or(0)
            + 1
    }

    /// The submission of this record that still waits for a decision, if any.
    pub(super) fn pending_submission(
        &self,
        store: &StateStore,
        scope: &ScopeId,
        requirement: &StableId,
    ) -> anyhow::Result<Option<CycleEntry>> {
        let dispositions = store.list_dispositions(scope)?;
        let decided: std::collections::BTreeSet<&str> = dispositions
            .iter()
            .map(|d| d.proposal_id.as_str())
            .collect();
        Ok(self
            .entries
            .iter()
            .filter(|e| e.requirement_id == *requirement && e.fact == CycleFact::Submitted)
            .filter(|e| {
                !decided.contains(e.proposal_id.as_str()) && !self.is_withdrawn(&e.proposal_id)
            })
            .cloned()
            .next_back())
    }
}

/// The proposal must be a bound review submission of a Requirement, because
/// the decision cycle addresses exactly that. Other proposals keep the
/// ordinary disposition paths, and a Question opens discussion without a
/// disposition.
pub(super) fn review_submission(
    store: &StateStore,
    scope: &ScopeId,
    proposal_id: &StableId,
) -> anyhow::Result<provenance_core::ProposalCard> {
    let proposal = store
        .list_proposal_definitions(scope)?
        .into_iter()
        .find(|p| p.id == *proposal_id)
        .ok_or_else(|| anyhow::anyhow!("review submission does not exist"))?;
    anyhow::ensure!(
        proposal.proposal_type == ProposalType::RecordRevision && proposal.record_revision.is_some(),
        "decision-cycle operations address record_revision submissions; other proposals keep their own disposition paths"
    );
    anyhow::ensure!(
        proposal.traceability.target.artifact_type == IdeationTargetType::Requirement,
        "the decision cycle currently addresses Requirements"
    );
    Ok(proposal)
}

/// A resubmission must answer exactly one rejection of one rejected
/// predecessor submission of the same record, and the rejection disposition it
/// answers is recorded on the fresh proposal.
pub(super) fn validated_resubmission(
    store: &StateStore,
    scope: &ScopeId,
    predecessor: &StableId,
    requirement: &StableId,
) -> anyhow::Result<StableId> {
    let proposals = store.list_proposal_definitions(scope)?;
    let predecessor = proposals
        .iter()
        .find(|p| p.id == *predecessor)
        .ok_or_else(|| anyhow::anyhow!("revises names no proposal"))?;
    anyhow::ensure!(
        predecessor.proposal_type == ProposalType::RecordRevision,
        "a resubmission revises a review submission"
    );
    anyhow::ensure!(
        predecessor.traceability.target.artifact_type == IdeationTargetType::Requirement
            && predecessor.traceability.target.artifact_id == *requirement,
        "a resubmission revises a submission of the same record"
    );
    let rejections: Vec<_> = store
        .list_dispositions(scope)?
        .into_iter()
        .filter(|d| d.proposal_id == predecessor.id)
        .collect();
    anyhow::ensure!(
        rejections.len() == 1,
        "a resubmission answers exactly one disposition of its predecessor"
    );
    anyhow::ensure!(
        rejections[0].decision == DispositionDecision::Rejected,
        "a resubmission answers a rejection"
    );
    Ok(rejections[0].id.clone())
}
