//! Shared decision-cycle helpers: the key and the response of a review
//! write, and the checks a submission or a decision must pass against the
//! scope. The review state of a record is in [`super::review_rows`].

use super::review_rows::CycleFacts;
use crate::state_store::StateStore;
use provenance_core::{
    review::{is_review_submission, review_cycle, CycleEntry, CycleFact, RecordReview},
    DispositionDecision, NodeType, ProposalCard, ScopeId, StableId,
};
use provenance_macros::rule;

const MAX_SAFE_CYCLE: u64 = (1 << 53) - 1;

pub(super) fn check_request_size(input: &impl serde::Serialize) -> anyhow::Result<()> {
    crate::write_error::ensure_request_size(serde_json::to_vec(input)?.len(), "review")
}

/// The key of the review Proposal for one record and one review cycle.
#[rule("rule_review_proposal_key_names_record_cycle")]
pub(super) fn review_proposal_key(kind: NodeType, record_id: &StableId, cycle: u64) -> String {
    format!("review:{}:{}:{cycle}", kind.as_str(), record_id.as_str())
}

/// The response of one decision-cycle write.
pub(super) struct Receipt {
    pub(super) id: StableId,
    pub(super) record_kind: NodeType,
    pub(super) record_id: StableId,
    pub(super) proposal: ProposalCard,
    pub(super) fact: CycleFact,
    pub(super) disposition_id: Option<StableId>,
    pub(super) feedback_message_id: Option<StableId>,
    pub(super) actor: String,
}

impl Receipt {
    /// Builds the response of a decision-cycle write.
    pub(super) fn entry(self) -> CycleEntry {
        CycleEntry {
            scope_id: self.proposal.scope_id.clone(),
            id: self.id,
            sequence: review_cycle(&self.proposal).unwrap_or_default(),
            proposal_key: (self.fact == CycleFact::Submitted)
                .then(|| self.proposal.proposal_key.clone()),
            record_kind: self.record_kind,
            record_id: self.record_id,
            proposal_id: self.proposal.id,
            fact: self.fact,
            disposition_id: self.disposition_id,
            feedback_message_id: self.feedback_message_id,
            actor: self.actor,
        }
    }
}

/// The review cycle of the next submission of one record.
pub(super) fn next_cycle(record: &RecordReview<'_>) -> anyhow::Result<u64> {
    let next = record
        .next_cycle()
        .ok_or_else(|| crate::refusal::fault!("review cycle overflow"))?;
    crate::write_error::ensure!(
        Fault,
        next <= MAX_SAFE_CYCLE,
        "review cycle exceeds the safe integer limit"
    );
    Ok(next)
}

/// The Proposal with this ID. The reading of the review rows decides
/// first, so a review submission whose record refuses gives the refusal of
/// that record.
fn proposal_named(
    store: &StateStore,
    scope: &ScopeId,
    facts: &CycleFacts<'_>,
    id: &StableId,
) -> anyhow::Result<Option<ProposalCard>> {
    if let Some(submission) = facts.submission(id)? {
        return Ok(Some(submission.clone()));
    }
    Ok(store
        .list_proposal_definitions(scope)?
        .into_iter()
        .find(|proposal| proposal.id == *id))
}

/// The proposal must be a bound record-review submission. Other proposals
/// keep the ordinary disposition paths.
pub(super) fn review_submission(
    store: &StateStore,
    scope: &ScopeId,
    facts: &CycleFacts<'_>,
    proposal_id: &StableId,
) -> anyhow::Result<ProposalCard> {
    let proposal = proposal_named(store, scope, facts, proposal_id)?.ok_or_else(|| {
        crate::refusal::refuse!(ResourceNotFound, "review submission does not exist")
    })?;
    crate::write_error::ensure!(
        InvalidUpdate,
        is_review_submission(&proposal) && proposal.record_revision.is_some(),
        "decision-cycle operations address record_revision submissions; other proposals keep their own disposition paths"
    );
    Ok(proposal)
}

/// A resubmission must answer the rejection of one rejected predecessor
/// submission of the same record, and the rejection it answers is recorded
/// on the fresh proposal. When the predecessor has more than one rejection,
/// the first in the order of the history is the one it answers.
pub(super) fn validated_resubmission(
    store: &StateStore,
    scope: &ScopeId,
    facts: &CycleFacts<'_>,
    predecessor: &StableId,
    kind: NodeType,
    record_id: &StableId,
) -> anyhow::Result<StableId> {
    let predecessor = proposal_named(store, scope, facts, predecessor)?
        .ok_or_else(|| crate::refusal::refuse!(MissingReference, "revises names no proposal"))?;
    crate::write_error::ensure!(
        InvalidUpdate,
        is_review_submission(&predecessor),
        "a resubmission revises a review submission"
    );
    crate::write_error::ensure!(
        InvalidUpdate,
        NodeType::from(predecessor.traceability.target.artifact_type) == kind
            && predecessor.traceability.target.artifact_id == *record_id,
        "a resubmission revises a submission of the same record"
    );
    let record = facts.record(kind, record_id)?;
    let answered = record
        .decisions()
        .into_iter()
        .find(|(_, proposal)| proposal.id == predecessor.id)
        .map(|(disposition, _)| disposition.id.clone());
    let Some(answered) = answered else {
        return Err(crate::refusal::refuse!(
            InvalidUpdate,
            "a resubmission answers a rejection of its predecessor"
        ));
    };
    crate::write_error::ensure!(
        InvalidUpdate,
        facts.agreed_decision(&predecessor.id) == Some(DispositionDecision::Rejected),
        "a resubmission answers a rejection"
    );
    Ok(answered)
}
