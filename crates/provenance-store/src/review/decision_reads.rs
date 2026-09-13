//! Current and historical decision reads for one record.
//!
//! The current read joins the bound submissions, their dispositions, the
//! cycle receipts, and the record's current review revision. Legacy unbound
//! dispositions keep their place in the history and stay authoritative for
//! their own proposals; none of them attests the record's current contents.

use super::decision_state::CycleFacts;
use crate::state_store::StateStore;
use provenance_core::{
    review::{PendingSubmission, RecordedDecision, RequirementDecisionState},
    DispositionDecision, IdeationTargetType, ProposalType, ScopeId, StableId,
};
use provenance_macros::rule;

impl StateStore {
    /// Reads the decision state of one Requirement: the submission still
    /// waiting, the acceptance that matches current content, every terminal
    /// decision, and every withdrawal.
    #[rule("rule_content_change_requires_new_review")]
    pub fn requirement_decision_state(
        &self,
        scope: &ScopeId,
        requirement_id: &StableId,
    ) -> anyhow::Result<RequirementDecisionState> {
        let record = self.requirement(scope, requirement_id)?;
        let head = self.head(&record)?;
        let proposals = self.list_proposal_definitions(scope)?;
        let dispositions = self.list_dispositions(scope)?;
        let facts = CycleFacts::validated(self, scope)?;
        let targets_record = |target: &provenance_core::IdeationTarget| {
            target.artifact_type == IdeationTargetType::Requirement
                && target.artifact_id == *requirement_id
        };
        let submissions: Vec<_> = proposals
            .iter()
            .filter(|p| {
                p.proposal_type == ProposalType::RecordRevision
                    && targets_record(&p.traceability.target)
            })
            .collect();
        let pending = facts
            .pending_submission(self, scope, requirement_id)?
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
                    && decision.revision.as_ref() == head.as_ref().map(|entry| &entry.revision)
            })
            .cloned();
        Ok(RequirementDecisionState {
            requirement_id: requirement_id.clone(),
            current_revision: head.map(|entry| entry.revision),
            pending,
            current_acceptance,
            decisions: recorded,
            withdrawn: facts.withdrawn_submissions(requirement_id),
        })
    }
}
