//! The Requirement candidate and decision cycle: submission, guarded
//! decisions with optional feedback, rejection with revision and resubmission,
//! and withdrawal.
//!
//! A decision never changes the record's lifecycle status. Acceptance accepts
//! the reviewed revision into the system description; rejection keeps a
//! nonempty rationale, may carry feedback, and returns the item to the agent;
//! a question keeps its own discussion paths and is never disposed here.

use super::{
    classifier,
    decision_input::{DecideRequirementReview, SubmitRequirementReview, WithdrawRequirementReview},
    decision_state::{
        request_digest, review_submission, validated_resubmission, write_receipt, CycleFacts,
    },
    guard, journal, owner_matches,
};
use crate::{
    publication::with_staged_state,
    shards,
    state_store::{CreateDispositionInput, CreateProposalCardInput, StateStore},
};
use provenance_core::{
    review::{CycleEntry, CycleFact, REVIEW_SCHEMA_VERSION},
    IdeationTarget, IdeationTargetType, NodeType, PromotionState, ProposalTraceability,
    ProposalType, RecordRevisionBinding, StableId, ThreadParent,
};
use provenance_macros::rule;
impl StateStore {
    /// Submits the record's current review revision as an immutable `proposed`
    /// candidate bound to that exact revision.
    ///
    /// One submission waits at a time. A submission after rejection names its
    /// rejected predecessor and answers its rejection; withdrawal is not a
    /// rejection, so a withdrawn submission is revised as a fresh candidate.
    #[rule("rule_revised_item_requires_new_review")]
    pub fn submit_requirement_review(
        &self,
        input: SubmitRequirementReview,
    ) -> anyhow::Result<CycleEntry> {
        anyhow::ensure!(
            !input.actor.trim().is_empty(),
            "invalid review request identity"
        );
        let digest = request_digest(&input)?;
        let scope = input.scope_id.clone();
        self.with_repository_publication(move || {
            anyhow::ensure!(
                self.manifest()?.scopes.iter().any(|s| s.id == scope),
                "review scope is not in the manifest"
            );
            if let Some(receipt) = super::decision_state::replay(
                self,
                &scope,
                &input.request_id,
                &input.actor,
                &digest,
            )? {
                return Ok(receipt);
            }
            let record = self.requirement(&scope, &input.requirement_id)?;
            owner_matches(&record, input.declared_by.as_deref())?;
            self.validated_review_entries(&scope)?;
            with_staged_state(&self.layout, false, |layout| {
                guard::with_writer(&shards::proposal_cards_path(layout, &scope), "*", || {
                    Self::new(layout.clone()).commit_submission(input, digest)
                })
            })
        })
    }

    fn commit_submission(
        &self,
        input: SubmitRequirementReview,
        digest: String,
    ) -> anyhow::Result<CycleEntry> {
        let scope = input.scope_id.clone();
        let record = self.requirement(&scope, &input.requirement_id)?;
        let head = self.head(&record)?.ok_or_else(|| {
            anyhow::anyhow!(
                "submission requires a review revision: save the record through the review seam first"
            )
        })?;
        if let Some(expected) = &input.expected_revision {
            anyhow::ensure!(
                head.revision == *expected,
                "stale submission: the record stands at revision {} and not at the expected revision {}",
                head.revision.as_str(),
                expected.as_str()
            );
        }
        let facts = CycleFacts::validated(self, &scope)?;
        anyhow::ensure!(
            facts
                .pending_submission(self, &scope, &input.requirement_id)?
                .is_none(),
            "this Requirement already has a pending review submission"
        );
        let (revises, revises_rejection) = match &input.revises {
            Some(predecessor) => {
                let rejection =
                    validated_resubmission(self, &scope, predecessor, &input.requirement_id)?;
                (Some(predecessor.clone()), Some(rejection))
            }
            None => (None, None),
        };
        let proposal = CreateProposalCardInput {
            scope_id: scope.clone(),
            id: input.proposal_id.clone(),
            proposal_key: input.proposal_key.clone(),
            proposal_type: ProposalType::RecordRevision,
            title: input.title.clone(),
            summary: input.summary.clone(),
            confidence: input.confidence,
            traceability: ProposalTraceability {
                target: IdeationTarget {
                    artifact_type: IdeationTargetType::Requirement,
                    artifact_id: input.requirement_id.clone(),
                },
                source_ids: input.source_ids.clone(),
                evidence_references: input.evidence_references.clone(),
                supporting_claim_ids: Vec::new(),
            },
            builds_on: input.builds_on.clone(),
            promotion_state: PromotionState::Proposed,
            duplicate_of: None,
            superseded_by: None,
            record_revision: Some(RecordRevisionBinding {
                revision: head.revision,
                content_digest: classifier::content_digest(&record)?,
            }),
            revises,
            revises_rejection,
        };
        self.create_proposal_card(proposal)?;
        let entry = CycleEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            sequence: facts.next_sequence(&input.requirement_id),
            scope_id: scope,
            id: journal::new_id(),
            requirement_id: input.requirement_id,
            proposal_id: input.proposal_id,
            fact: CycleFact::Submitted,
            disposition_id: None,
            feedback_message_id: None,
            actor: input.actor,
            request_id: input.request_id,
            intent_digest: digest,
        };
        write_receipt(self, &entry)?;
        Ok(entry)
    }

    /// Records one guarded Disposition on a review submission.
    ///
    /// Every gate the ordinary disposition write applies stays in force: the
    /// actor allowlist, one terminal Disposition per Proposal, Assertion
    /// qualification for an acceptance, and the human existing-artifact
    /// exception. On top of them, a decision against a superseded revision is
    /// refused: the submission must bind the record's current revision and
    /// content. Optional feedback is a Discussion comment published in the
    /// same commit, so a decision with feedback publishes both or neither.
    #[rule("rule_approval_accepts_reviewed_version")]
    pub fn decide_requirement_review(
        &self,
        input: DecideRequirementReview,
    ) -> anyhow::Result<CycleEntry> {
        let actor = input.actor.id.clone();
        let digest = request_digest(&input)?;
        let scope = input.scope_id.clone();
        self.with_repository_publication(move || {
            anyhow::ensure!(
                self.manifest()?.scopes.iter().any(|s| s.id == scope),
                "review scope is not in the manifest"
            );
            if let Some(receipt) =
                super::decision_state::replay(self, &scope, &input.request_id, &actor, &digest)?
            {
                return Ok(receipt);
            }
            let proposal = review_submission(self, &scope, &input.proposal_id)?;
            self.requirement(&scope, &proposal.traceability.target.artifact_id)?;
            let facts = CycleFacts::validated(self, &scope)?;
            anyhow::ensure!(
                !facts.is_withdrawn(&input.proposal_id),
                "this review submission was withdrawn from review"
            );
            with_staged_state(&self.layout, false, |layout| {
                Self::new(layout.clone()).commit_decision(input, digest)
            })
        })
    }

    fn commit_decision(
        &self,
        input: DecideRequirementReview,
        digest: String,
    ) -> anyhow::Result<CycleEntry> {
        let DecideRequirementReview {
            scope_id: scope,
            request_id,
            actor,
            proposal_id,
            disposition_id,
            decision,
            rationale,
            canonical_artifact,
            feedback,
            declared_by,
        } = input;
        let proposal = review_submission(self, &scope, &proposal_id)?;
        let requirement_id = proposal.traceability.target.artifact_id.clone();
        let binding = proposal
            .record_revision
            .as_ref()
            .expect("review_submission checks the binding");
        let record = self.requirement(&scope, &requirement_id)?;
        let head = self
            .head(&record)?
            .ok_or_else(|| anyhow::anyhow!("the submitted record has no review history"))?;
        anyhow::ensure!(
            head.revision == binding.revision
                && classifier::content_digest(&record)? == binding.content_digest,
            "stale review selection: the submission binds revision {} but the record stands at \
             revision {}; withdraw it and submit the current revision",
            binding.revision.as_str(),
            head.revision.as_str()
        );
        guard::with_writer(
            &shards::dispositions_path(&self.layout, &scope),
            "*",
            || {
                self.create_disposition(CreateDispositionInput {
                    scope_id: scope.clone(),
                    id: disposition_id.clone(),
                    proposal_id: proposal_id.clone(),
                    decision,
                    rationale,
                    actor: actor.clone(),
                    canonical_artifact,
                    external_action: None,
                })
            },
        )?;
        let feedback_message_id = match feedback {
            Some(feedback) => Some(self.publish_feedback(
                &scope,
                &requirement_id,
                &actor,
                declared_by.as_deref(),
                &request_id,
                feedback,
            )?),
            None => None,
        };
        let entry = CycleEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            sequence: CycleFacts::validated(self, &scope)?.next_sequence(&requirement_id),
            scope_id: scope,
            id: journal::new_id(),
            requirement_id,
            proposal_id,
            fact: CycleFact::Decided,
            disposition_id: Some(disposition_id),
            feedback_message_id,
            actor: actor.id,
            request_id,
            intent_digest: digest,
        };
        write_receipt(self, &entry)?;
        Ok(entry)
    }

    /// Publishes the decision's optional feedback as a new Discussion root and
    /// returns the Message it produced. Runs inside the decision's staged
    /// publication, so a refusal here also refuses the decision.
    #[rule("rule_rejection_comment_is_optional")]
    fn publish_feedback(
        &self,
        scope: &provenance_core::ScopeId,
        requirement_id: &provenance_core::StableId,
        actor: &provenance_core::DispositionActor,
        declared_by: Option<&str>,
        request_id: &provenance_core::StableId,
        feedback: super::decision_input::ReviewFeedback,
    ) -> anyhow::Result<provenance_core::StableId> {
        use super::discussion_input::{DiscussionAction, WriteDiscussion};

        let discussion = WriteDiscussion {
            scope_id: scope.clone(),
            parent: ThreadParent {
                node_type: NodeType::Requirement,
                node_id: requirement_id.clone(),
            },
            request_id: feedback_request_id(request_id)?,
            actor: actor.id.clone(),
            declared_by: declared_by.map(str::to_string),
            action: DiscussionAction::Start {
                role: feedback.role,
                body: feedback.body,
            },
        };
        let digest = super::discussion_writes::intent(&discussion)?;
        self.authorize_discussion(&discussion)?;
        let entry = guard::with_writer(&shards::threads_path(&self.layout, scope), "*", || {
            guard::with_writer(&shards::messages_path(&self.layout, scope), "*", || {
                self.commit_discussion(discussion, None, digest)
            })
        })?;
        entry
            .message_id
            .ok_or_else(|| anyhow::anyhow!("feedback published no Message"))
    }

    /// Withdraws a pending submission from review. The candidate, its
    /// feedback, and the graph record all stay; nothing is retired or deleted.
    #[rule("rule_withdrawal_preserves_review_history")]
    pub fn withdraw_requirement_review(
        &self,
        input: WithdrawRequirementReview,
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
            if let Some(receipt) =
                super::decision_state::replay(self, &scope, &input.request_id, &input.actor, &digest)?
            {
                return Ok(receipt);
            }
            let proposal = review_submission(self, &scope, &input.proposal_id)?;
            let record = self.requirement(&scope, &proposal.traceability.target.artifact_id)?;
            owner_matches(&record, input.declared_by.as_deref())?;
            let facts = CycleFacts::validated(self, &scope)?;
            anyhow::ensure!(
                !facts.is_withdrawn(&input.proposal_id),
                "this review submission was already withdrawn"
            );
            anyhow::ensure!(
                !facts.is_decided(&input.proposal_id),
                "this review submission already has a decision; withdrawal applies to a pending submission"
            );
            with_staged_state(&self.layout, false, |layout| {
                Self::new(layout.clone()).commit_withdrawal(input, digest)
            })
        })
    }

    fn commit_withdrawal(
        &self,
        input: WithdrawRequirementReview,
        digest: String,
    ) -> anyhow::Result<CycleEntry> {
        let scope = input.scope_id.clone();
        let proposal = review_submission(self, &scope, &input.proposal_id)?;
        let requirement_id = proposal.traceability.target.artifact_id;
        let entry = CycleEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            sequence: CycleFacts::validated(self, &scope)?.next_sequence(&requirement_id),
            scope_id: scope,
            id: journal::new_id(),
            requirement_id,
            proposal_id: input.proposal_id,
            fact: CycleFact::Withdrawn,
            disposition_id: None,
            feedback_message_id: None,
            actor: input.actor,
            request_id: input.request_id,
            intent_digest: digest,
        };
        write_receipt(self, &entry)?;
        Ok(entry)
    }
}

/// The feedback Discussion's request key is derived from the decision's, so a
/// replayed decision replays to the same feedback.
fn feedback_request_id(
    request: &provenance_core::StableId,
) -> anyhow::Result<provenance_core::StableId> {
    StableId::new(format!("{}_feedback", request.as_str()))
}
