use super::target_type;
use crate::{
    review::publication::with_record_state,
    review::{
        classifier,
        decision_input::SubmitRecordReview,
        decision_state::{
            check_request_size, review_proposal_key, validated_resubmission, CycleFacts, Receipt,
        },
        guard, new_id, owner_matches,
    },
    shards,
    state_store::{CreateProposalCardInput, StateStore},
    write_error::SourceFailure,
};
use provenance_core::{
    review::{CycleEntry, CycleFact},
    IdeationTarget, PromotionState, ProposalTraceability, ProposalType, RecordRevisionBinding,
};
use provenance_macros::rule;

impl StateStore {
    /// Creates the server-owned Proposal identity for a review submission.
    #[rule("rule_revised_item_requires_new_review")]
    #[rule("rule_review_proposal_identity_server_created")]
    pub fn submit_record_review(&self, input: SubmitRecordReview) -> anyhow::Result<CycleEntry> {
        anyhow::ensure!(
            !input.actor.trim().is_empty(),
            "invalid review request identity"
        );
        check_request_size(&input)?;
        let scope = input.scope_id.clone();
        self.with_repository_publication(move || {
            anyhow::ensure!(
                self.manifest()?.scopes.iter().any(|s| s.id == scope),
                "review scope is not in the manifest"
            );
            let record = crate::cache::review_families::record(
                self,
                &scope,
                input.record_kind,
                &input.record_id,
            )?;
            owner_matches(&record, input.declared_by.as_deref())?;
            with_record_state(&self.layout, |layout| {
                guard::with_writer(&shards::proposal_cards_path(layout, &scope), "*", || {
                    Self::new(layout.clone()).commit_submission(input)
                })
            })
        })
    }

    pub(in crate::review) fn commit_submission(
        &self,
        input: SubmitRecordReview,
    ) -> anyhow::Result<CycleEntry> {
        let scope = input.scope_id.clone();
        let record = crate::cache::review_families::record(
            self,
            &scope,
            input.record_kind,
            &input.record_id,
        )?;
        let revision = super::super::save::current_revision(&record)?.ok_or_else(|| {
            anyhow::anyhow!(
                "submission requires a review revision: save the record through the review seam first"
            )
        })?;
        let facts = CycleFacts::validated(self, &scope)?;
        if input
            .expected_revision
            .as_ref()
            .is_some_and(|expected| revision != *expected)
        {
            return Err(SourceFailure::wrap(
                facts.conflict_failure(self, &scope, input.record_kind, &input.record_id)?,
                anyhow::anyhow!("stale submission revision"),
            ));
        }
        if facts
            .pending_submission(self, &scope, input.record_kind, &input.record_id)?
            .is_some()
        {
            return Err(SourceFailure::wrap(
                facts.conflict_failure(self, &scope, input.record_kind, &input.record_id)?,
                anyhow::anyhow!("this record already has a pending review submission"),
            ));
        }
        let (revises, revises_rejection) = match &input.revises {
            Some(predecessor) => (
                Some(predecessor.clone()),
                Some(validated_resubmission(
                    self,
                    &scope,
                    predecessor,
                    input.record_kind,
                    &input.record_id,
                )?),
            ),
            None => (None, None),
        };
        let cycle = facts.next_cycle(input.record_kind, &input.record_id)?;
        let proposal = self.create_proposal_card(CreateProposalCardInput {
            scope_id: scope,
            id: new_id(),
            proposal_key: review_proposal_key(input.record_kind, &input.record_id, cycle),
            proposal_type: ProposalType::RecordRevision,
            title: input.title,
            summary: input.summary,
            confidence: input.confidence,
            traceability: ProposalTraceability {
                target: IdeationTarget {
                    artifact_type: target_type(input.record_kind),
                    artifact_id: input.record_id.clone(),
                },
                source_ids: input.source_ids,
                evidence_references: input.evidence_references,
                supporting_claim_ids: Vec::new(),
            },
            builds_on: input.builds_on,
            promotion_state: PromotionState::Proposed,
            duplicate_of: None,
            superseded_by: None,
            record_revision: Some(RecordRevisionBinding {
                revision,
                content_digest: classifier::content_digest(input.record_kind, &record)?,
            }),
            actor: Some(input.actor.clone()),
            revises,
            revises_rejection,
        })?;
        Ok(Receipt {
            id: proposal.id.clone(),
            record_kind: input.record_kind,
            record_id: input.record_id,
            proposal,
            fact: CycleFact::Submitted,
            disposition_id: None,
            feedback_message_id: None,
            actor: input.actor,
        }
        .entry())
    }
}
