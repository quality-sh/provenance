use super::validate_submission_address;
use crate::{
    publication::with_staged_state,
    review::{
        classifier,
        decision_input::WithdrawRecordReview,
        decision_state::{request_digest, review_submission, write_receipt, CycleFacts},
        journal, owner_matches,
    },
    state_store::StateStore,
    write_error::SourceFailure,
};
use provenance_core::{
    review::{CycleEntry, CycleFact, REVIEW_SCHEMA_VERSION},
    NodeType, StableId,
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
        let request_id = crate::review::new_request_id();
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
                &facts,
                &input.proposal_id,
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
            let head = self
                .head(&record)?
                .ok_or_else(|| anyhow::anyhow!("the submitted record has no review history"))?;
            if head.revision != binding.revision
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
                Self::new(layout.clone()).commit_withdrawal(input, request_id, digest)
            })
        })
    }

    fn commit_withdrawal(
        &self,
        input: WithdrawRecordReview,
        request_id: StableId,
        digest: String,
    ) -> anyhow::Result<CycleEntry> {
        let proposal = review_submission(self, &input.scope_id, &input.proposal_id)?;
        let kind = NodeType::from(proposal.traceability.target.artifact_type);
        let record_id = proposal.traceability.target.artifact_id;
        let entry = CycleEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            sequence: CycleFacts::validated(self, &input.scope_id)?
                .next_sequence(kind, &record_id)?,
            scope_id: input.scope_id,
            id: journal::new_id(),
            record_kind: kind,
            record_id,
            proposal_id: input.proposal_id,
            proposal_key: None,
            fact: CycleFact::Withdrawn,
            disposition_id: None,
            feedback_message_id: None,
            actor: input.actor,
            request_id,
            intent_digest: digest,
        };
        write_receipt(self, &entry)?;
        Ok(entry)
    }
}
