//! Current and historical decision reads for one record.
//!
//! The current read joins the bound submissions, their dispositions and
//! withdrawals, the feedback Discussions, and the record's current review
//! revision. Legacy unbound
//! dispositions keep their place in the history and stay authoritative for
//! their own proposals; none of them attests the record's current contents.

use super::decision_state::CycleFacts;
use crate::{cache::review_families::ReviewFamilyIndex, state_store::StateStore};
use provenance_core::{
    protocol::DocumentReviewSummary,
    review::{PendingSubmission, RecordedDecision, RequirementDecisionState, ReviewRecord},
    DispositionDecision, NodeType, ScopeId, StableId,
};
use provenance_macros::rule;

pub struct DocumentReviewState {
    pub records: Vec<(NodeType, StableId, DocumentReviewSummary)>,
    /// The records whose review rejected them before any acceptance.
    pub rejected_new: Vec<(NodeType, StableId)>,
    pub digest: String,
}

impl DocumentReviewState {
    pub fn summary(&self, kind: NodeType, id: &StableId) -> DocumentReviewSummary {
        self.records
            .iter()
            .find(|(record_kind, record_id, _)| *record_kind == kind && record_id == id)
            .map(|(_, _, summary)| summary)
            .cloned()
            .unwrap_or_default()
    }
}

/// A record that no review accepted and whose latest review decision rejected
/// it is a rejected new record. A rejected change to an accepted record is
/// not. Only decisions on review submissions count: a decision on a
/// proposal of another type that names the record, such as a candidate
/// answer to a Question, decides nothing about the record.
#[rule("rule_review_document_hides_rejected_new_records")]
fn rejected_before_acceptance(state: &RequirementDecisionState, facts: &CycleFacts) -> bool {
    let reviews = state
        .decisions
        .iter()
        .filter(|decision| facts.is_submission(&decision.disposition.proposal_id))
        .collect::<Vec<_>>();
    state.pending.is_none()
        && reviews
            .last()
            .is_some_and(|decision| decision.disposition.decision == DispositionDecision::Rejected)
        && !reviews
            .iter()
            .any(|decision| decision.disposition.decision == DispositionDecision::Accepted)
}

impl StateStore {
    /// Builds the current review outcome for each entry and the totals for the document.
    #[rule("rule_document_entry_has_review_outcome")]
    #[rule("rule_document_has_review_totals")]
    pub(crate) fn document_review_state(
        &self,
        scope: &ScopeId,
        records: &[(NodeType, StableId)],
    ) -> anyhow::Result<DocumentReviewState> {
        self.with_repository_publication(|| {
            let facts = CycleFacts::validated(self, scope)?;
            let mut families = ReviewFamilyIndex::new(self, scope, records);
            let mut states = Vec::new();
            for (kind, id) in records {
                // A reference to a record that no longer exists has no review state.
                if let Some(record) = families.find(*kind, id)? {
                    states.push(Self::record_decision_state_from_parts(record, &facts)?);
                }
            }
            let digest = crate::canonical_digest::digest(
                &crate::canonical_digest::canonical_bytes(&states)?,
            );
            let rejected_new = states
                .iter()
                .filter(|state| rejected_before_acceptance(state, &facts))
                .map(|state| (state.record_kind, state.record_id.clone()))
                .collect();
            let records = states
                .iter()
                .map(|state| {
                    (
                        state.record_kind,
                        state.record_id.clone(),
                        DocumentReviewSummary::from(state),
                    )
                })
                .collect::<Vec<_>>();
            Ok(DocumentReviewState {
                records,
                rejected_new,
                digest,
            })
        })
    }

    /// Reads the decision state of one Requirement: the submission still
    /// waiting, the acceptance that matches current content, every terminal
    /// decision, and every withdrawal.
    pub fn requirement_decision_state(
        &self,
        scope: &ScopeId,
        requirement_id: &StableId,
    ) -> anyhow::Result<RequirementDecisionState> {
        self.record_decision_state(scope, NodeType::Requirement, requirement_id)
    }

    pub fn record_decision_state(
        &self,
        scope: &ScopeId,
        kind: NodeType,
        record_id: &StableId,
    ) -> anyhow::Result<RequirementDecisionState> {
        let record = crate::cache::review_families::record(self, scope, kind, record_id)?;
        self.record_decision_state_for_record(&record)
    }

    pub(super) fn record_decision_state_for_record(
        &self,
        record: &ReviewRecord,
    ) -> anyhow::Result<RequirementDecisionState> {
        let facts = CycleFacts::validated(self, record.scope_id())?;
        Self::record_decision_state_from_parts(record, &facts)
    }

    /// The decision state of one record from one read of the scope's
    /// review records.
    fn record_decision_state_from_parts(
        record: &ReviewRecord,
        facts: &CycleFacts,
    ) -> anyhow::Result<RequirementDecisionState> {
        let proposals = facts.proposals();
        let dispositions = facts.dispositions();
        let kind = record.kind();
        let record_id = record.id();
        let current_revision = super::save::current_revision(record)?;
        let targets_record = |target: &provenance_core::IdeationTarget| {
            NodeType::from(target.artifact_type) == kind && target.artifact_id == *record_id
        };
        let pending = facts
            .pending_submission_at_revision(kind, record_id, current_revision.as_ref())
            .map(|proposal| PendingSubmission {
                proposal_id: proposal.id,
                revision: proposal
                    .record_revision
                    .expect("pending submissions carry bindings")
                    .revision,
                actor: proposal.actor.unwrap_or_default(),
                revises: proposal.revises,
            });
        let mut recorded: Vec<RecordedDecision> = dispositions
            .iter()
            .filter_map(|disposition| {
                let proposal = proposals.iter().find(|p| p.id == disposition.proposal_id)?;
                if !targets_record(&proposal.traceability.target) {
                    return None;
                }
                Some(RecordedDecision {
                    revision: proposal
                        .record_revision
                        .as_ref()
                        .map(|binding| binding.revision.clone()),
                    feedback_message_id: facts.feedback_for(&disposition.id),
                    disposition: disposition.clone(),
                })
            })
            .collect();
        recorded.sort_by_key(|decision| facts.order_of(&decision.disposition.proposal_id));
        let current_acceptance = recorded
            .iter()
            .filter(|decision| decision.disposition.decision == DispositionDecision::Accepted)
            .find(|decision| {
                decision.revision.is_some()
                    && decision.revision.as_ref() == current_revision.as_ref()
            })
            .cloned();
        Ok(RequirementDecisionState {
            record_kind: kind,
            record_id: record_id.clone(),
            current_revision,
            pending,
            current_acceptance,
            decisions: recorded,
            withdrawn: facts.withdrawn_submissions(kind, record_id),
        })
    }
}
