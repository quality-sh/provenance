use super::target_type;
use crate::{
    publication::with_staged_state,
    review::{
        classifier,
        decision_input::SubmitRecordReview,
        decision_state::{request_digest, validated_resubmission, write_receipt, CycleFacts},
        guard, journal, owner_matches,
    },
    shards,
    state_store::{CreateProposalCardInput, StateStore},
    write_error::SourceFailure,
};
use provenance_core::{
    review::{CycleEntry, CycleFact, REVIEW_SCHEMA_VERSION},
    IdeationTarget, PromotionState, ProposalTraceability, ProposalType, RecordRevisionBinding,
    StableId,
};
use provenance_macros::rule;

impl StateStore {
    #[rule("rule_revised_item_requires_new_review")]
    pub fn submit_record_review(&self, input: SubmitRecordReview) -> anyhow::Result<CycleEntry> {
        anyhow::ensure!(
            !input.actor.trim().is_empty(),
            "invalid review request identity"
        );
        let digest = request_digest(&input)?;
        let request_id = journal::new_id();
        let proposal_id = journal::new_id();
        let proposal_key = proposal_id.as_str().to_owned();
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
            self.validated_review_entries(&scope)?;
            with_staged_state(&self.layout, false, |layout| {
                guard::with_writer(&shards::proposal_cards_path(layout, &scope), "*", || {
                    Self::new(layout.clone()).commit_submission(
                        input,
                        request_id,
                        proposal_id,
                        proposal_key,
                        digest,
                    )
                })
            })
        })
    }

    pub(in crate::review) fn commit_submission(
        &self,
        input: SubmitRecordReview,
        request_id: StableId,
        proposal_id: StableId,
        proposal_key: String,
        digest: String,
    ) -> anyhow::Result<CycleEntry> {
        let scope = input.scope_id.clone();
        let record = crate::cache::review_families::record(
            self,
            &scope,
            input.record_kind,
            &input.record_id,
        )?;
        let head = self.head(&record)?.ok_or_else(|| {
            anyhow::anyhow!(
                "submission requires a review revision: save the record through the review seam first"
            )
        })?;
        let facts = CycleFacts::validated(self, &scope)?;
        if input
            .expected_revision
            .as_ref()
            .is_some_and(|expected| head.revision != *expected)
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
        self.create_proposal_card(CreateProposalCardInput {
            scope_id: scope.clone(),
            id: proposal_id.clone(),
            proposal_key: proposal_key.clone(),
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
                revision: head.revision,
                content_digest: classifier::content_digest(input.record_kind, &record)?,
            }),
            revises,
            revises_rejection,
        })?;
        let entry = CycleEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            sequence: facts.next_sequence(input.record_kind, &input.record_id)?,
            scope_id: scope,
            id: journal::new_id(),
            record_kind: input.record_kind,
            record_id: input.record_id,
            proposal_id,
            proposal_key: Some(proposal_key),
            fact: CycleFact::Submitted,
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
