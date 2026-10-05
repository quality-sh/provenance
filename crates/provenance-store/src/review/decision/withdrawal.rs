use super::validate_submission_address;
use crate::{
    publication::with_staged_state,
    review::{
        classifier,
        decision_input::WithdrawRecordReview,
        decision_state::{request_digest, review_submission, CycleFacts, Receipt},
        guard, new_id, owner_matches,
    },
    shards,
    state_store::StateStore,
    write_error::SourceFailure,
};
use provenance_core::{
    review::{CycleEntry, CycleFact},
    NodeType, StableId, Withdrawal, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::rule;

impl StateStore {
    #[rule("rule_withdrawal_preserves_review_history")]
    pub fn withdraw_record_review(
        &self,
        input: WithdrawRecordReview,
    ) -> anyhow::Result<CycleEntry> {
        self.withdraw_record_review_addressed(None, input)
    }

    pub fn withdraw_record_review_for(
        &self,
        kind: NodeType,
        record_id: &StableId,
        input: WithdrawRecordReview,
    ) -> anyhow::Result<CycleEntry> {
        self.withdraw_record_review_addressed(Some((kind, record_id.clone())), input)
    }

    fn withdraw_record_review_addressed(
        &self,
        addressed: Option<(NodeType, StableId)>,
        input: WithdrawRecordReview,
    ) -> anyhow::Result<CycleEntry> {
        anyhow::ensure!(
            !input.actor.trim().is_empty(),
            "invalid review request identity"
        );
        if let Some(reason) = &input.reason {
            anyhow::ensure!(
                !reason.trim().is_empty(),
                "withdrawal reason must not be empty"
            );
        }
        let digest = request_digest(&input)?;
        let scope = input.scope_id.clone();
        self.with_repository_publication(move || {
            anyhow::ensure!(
                self.manifest()?.scopes.iter().any(|s| s.id == scope),
                "review scope is not in the manifest"
            );
            let proposal = review_submission(self, &scope, &input.proposal_id)?;
            let facts = CycleFacts::validated(self, &scope)?;
            validate_submission_address(
                &proposal,
                addressed.as_ref().map(|(kind, id)| (*kind, id)),
            )?;
            let kind = NodeType::from(proposal.traceability.target.artifact_type);
            let record_id = &proposal.traceability.target.artifact_id;
            let record = crate::cache::review_families::record(self, &scope, kind, record_id)?;
            owner_matches(&record, input.declared_by.as_deref())?;
            let binding = proposal
                .record_revision
                .as_ref()
                .expect("review_submission checks the binding");
            let revision = super::super::save::current_revision(&record)?
                .ok_or_else(|| anyhow::anyhow!("the submitted record has no review revision"))?;
            if revision != binding.revision
                || classifier::content_digest(kind, &record)? != binding.content_digest
                || facts.is_withdrawn(&input.proposal_id)
                || facts.is_decided(&input.proposal_id)
            {
                return Err(SourceFailure::wrap(
                    facts.conflict_failure(self, &scope, kind, record_id)?,
                    anyhow::anyhow!("this review submission is no longer current and pending"),
                ));
            }
            with_staged_state(&self.layout, false, |layout| {
                Self::new(layout.clone()).commit_withdrawal(input, digest)
            })
        })
    }

    /// Writes the Withdrawal record of one review submission.
    fn commit_withdrawal(
        &self,
        input: WithdrawRecordReview,
        digest: String,
    ) -> anyhow::Result<CycleEntry> {
        let proposal = review_submission(self, &input.scope_id, &input.proposal_id)?;
        let kind = NodeType::from(proposal.traceability.target.artifact_type);
        let record_id = proposal.traceability.target.artifact_id.clone();
        let withdrawal = guard::with_writer(
            &shards::withdrawals_path(&self.layout, &input.scope_id),
            "*",
            || {
                self.create_withdrawal(Withdrawal {
                    schema_version: SUPPORTED_SCHEMA_VERSION,
                    scope_id: input.scope_id.clone(),
                    id: new_id(),
                    proposal_id: input.proposal_id.clone(),
                    actor: input.actor.clone(),
                    reason: input.reason.clone(),
                })
            },
        )?;
        Ok(Receipt {
            id: withdrawal.id,
            record_kind: kind,
            record_id,
            proposal,
            fact: CycleFact::Withdrawn,
            disposition_id: None,
            feedback_message_id: None,
            actor: input.actor,
            request_id: new_id(),
            intent_digest: digest,
        }
        .entry())
    }
}
