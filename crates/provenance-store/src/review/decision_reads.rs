//! Current and historical decision reads for one record.
//!
//! The current read joins the bound submissions, their dispositions, the
//! cycle receipts, and the record's current review revision. Legacy unbound
//! dispositions keep their place in the history and stay authoritative for
//! their own proposals; none of them attests the record's current contents.

use super::decision_state::CycleFacts;
use crate::state_store::StateStore;
use provenance_core::{
    protocol::{DocumentReviewSummary, DocumentReviewTotals},
    review::{PendingSubmission, RecordedDecision, RequirementDecisionState},
    DispositionDecision, DispositionRecord, NodeType, ProposalCard, ProposalType, ScopeId,
    StableId,
};
use provenance_macros::rule;

pub struct DocumentReviewState {
    pub records: Vec<(NodeType, StableId, DocumentReviewSummary)>,
    pub totals: DocumentReviewTotals,
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
            let proposals = self.list_proposal_definitions(scope)?;
            let dispositions = self.list_dispositions(scope)?;
            let facts = CycleFacts::validated(self, scope)?;
            let states = records
                .iter()
                .map(|(kind, id)| {
                    let record = crate::cache::review_families::record(self, scope, *kind, id)?;
                    self.record_decision_state_from_parts(
                        &record,
                        &proposals,
                        &dispositions,
                        &facts,
                    )
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let digest = crate::canonical_digest::digest(
                &crate::canonical_digest::canonical_bytes(&states)?,
            );
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
            let mut totals = DocumentReviewTotals::default();
            for (_, _, summary) in &records {
                totals.include(summary);
            }
            Ok(DocumentReviewState {
                records,
                totals,
                digest,
            })
        })
    }

    /// Reads the decision state of one Requirement: the submission still
    /// waiting, the acceptance that matches current content, every terminal
    /// decision, and every withdrawal.
    #[rule("rule_content_change_requires_new_review")]
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
        record: &provenance_core::review::ReviewRecord,
    ) -> anyhow::Result<RequirementDecisionState> {
        let scope = record.scope_id();
        let proposals = self.list_proposal_definitions(scope)?;
        let dispositions = self.list_dispositions(scope)?;
        let facts = CycleFacts::validated(self, scope)?;
        self.record_decision_state_from_parts(record, &proposals, &dispositions, &facts)
    }

    fn record_decision_state_from_parts(
        &self,
        record: &provenance_core::review::ReviewRecord,
        proposals: &[ProposalCard],
        dispositions: &[DispositionRecord],
        facts: &CycleFacts,
    ) -> anyhow::Result<RequirementDecisionState> {
        let kind = record.kind();
        let record_id = record.id();
        let head = self.head(record)?;
        let current_revision = head.as_ref().map(|entry| entry.revision.clone());
        let targets_record = |target: &provenance_core::IdeationTarget| {
            NodeType::from(target.artifact_type) == kind && target.artifact_id == *record_id
        };
        let submissions: Vec<_> = proposals
            .iter()
            .filter(|p| {
                p.proposal_type == ProposalType::RecordRevision
                    && targets_record(&p.traceability.target)
            })
            .collect();
        let pending = facts
            .pending_submission_at_revision_in(
                proposals,
                dispositions,
                kind,
                record_id,
                current_revision.as_ref(),
            )
            .map(|entry| {
                let proposal = submissions
                    .iter()
                    .find(|p| p.id == entry.proposal_id)
                    .expect("validated cycle entries name existing proposals");
                PendingSubmission {
                    proposal_id: proposal.id.clone(),
                    revision: proposal
                        .record_revision
                        .as_ref()
                        .expect("submissions carry bindings")
                        .revision
                        .clone(),
                    actor: entry.actor,
                    revises: proposal.revises.clone(),
                }
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
        recorded.sort_by_key(|decision| {
            facts
                .sequence_of(&decision.disposition.proposal_id)
                .unwrap_or(u64::MAX)
        });
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
