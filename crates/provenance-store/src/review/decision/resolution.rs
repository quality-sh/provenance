use super::{feedback_request_id, validate_submission_address};
use crate::{
    publication::with_staged_state,
    review::{
        classifier,
        decision_input::{DecideRecordReview, DecideRequirementReview, ReviewFeedback},
        decision_state::{request_digest, review_submission, write_receipt, CycleFacts},
        guard, journal,
    },
    shards,
    state_store::{CreateDispositionInput, StateStore},
    write_error::{SourceFailure, WriteFailure},
};
use provenance_core::{
    review::{CycleEntry, CycleFact, REVIEW_SCHEMA_VERSION},
    NodeType, StableId, ThreadParent,
};
use provenance_macros::rule;

impl StateStore {
    #[rule("rule_approval_accepts_reviewed_version")]
    pub fn decide_record_review(&self, input: DecideRecordReview) -> anyhow::Result<CycleEntry> {
        self.decide_record_review_addressed(None, input)
    }

    pub fn decide_requirement_review(
        &self,
        input: DecideRequirementReview,
    ) -> anyhow::Result<CycleEntry> {
        self.decide_record_review(input)
    }

    pub fn decide_record_review_for(
        &self,
        kind: NodeType,
        record_id: &StableId,
        input: DecideRecordReview,
    ) -> anyhow::Result<CycleEntry> {
        self.decide_record_review_addressed(Some((kind, record_id.clone())), input)
    }

    pub fn decide_requirement_review_for(
        &self,
        requirement_id: &StableId,
        input: DecideRequirementReview,
    ) -> anyhow::Result<CycleEntry> {
        self.decide_record_review_for(NodeType::Requirement, requirement_id, input)
    }

    fn decide_record_review_addressed(
        &self,
        addressed: Option<(NodeType, StableId)>,
        input: DecideRecordReview,
    ) -> anyhow::Result<CycleEntry> {
        let digest = request_digest(&input)?;
        let request_id = journal::new_id();
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
            crate::cache::review_families::record(self, &scope, kind, record_id)?;
            if input.feedback.is_some() && matches!(kind, NodeType::Domain | NodeType::Boundary) {
                return Err(SourceFailure::wrap(
                    WriteFailure::UnsupportedReviewFeedback { record_kind: kind },
                    anyhow::anyhow!("this record kind does not support review feedback"),
                ));
            }
            if facts.is_withdrawn(&input.proposal_id) || facts.is_decided(&input.proposal_id) {
                return Err(SourceFailure::wrap(
                    facts.conflict_failure(self, &scope, kind, record_id)?,
                    anyhow::anyhow!("this review submission is no longer pending"),
                ));
            }
            with_staged_state(&self.layout, false, |layout| {
                Self::new(layout.clone()).commit_decision(input, request_id, digest)
            })
        })
    }

    fn commit_decision(
        &self,
        input: DecideRecordReview,
        request_id: StableId,
        digest: String,
    ) -> anyhow::Result<CycleEntry> {
        let proposal = review_submission(self, &input.scope_id, &input.proposal_id)?;
        let kind = NodeType::from(proposal.traceability.target.artifact_type);
        let record_id = proposal.traceability.target.artifact_id.clone();
        let binding = proposal
            .record_revision
            .as_ref()
            .expect("review_submission checks the binding");
        let record = crate::cache::review_families::record(
            self,
            &input.scope_id,
            kind,
            &record_id,
        )?;
        let head = self
            .head(&record)?
            .ok_or_else(|| anyhow::anyhow!("the submitted record has no review history"))?;
        if head.revision != binding.revision
            || classifier::content_digest(kind, &record)? != binding.content_digest
        {
            let facts = CycleFacts::validated(self, &input.scope_id)?;
            return Err(SourceFailure::wrap(
                facts.conflict_failure(self, &input.scope_id, kind, &record_id)?,
                anyhow::anyhow!("stale review selection"),
            ));
        }
        let disposition_id = journal::new_id();
        guard::with_writer(
            &shards::dispositions_path(&self.layout, &input.scope_id),
            "*",
            || {
                self.create_disposition(CreateDispositionInput {
                    scope_id: input.scope_id.clone(),
                    id: disposition_id.clone(),
                    proposal_id: input.proposal_id.clone(),
                    decision: input.decision,
                    rationale: input.rationale.clone().unwrap_or_default(),
                    actor: input.actor.clone(),
                    canonical_artifact: input.canonical_artifact.clone(),
                    external_action: None,
                })
            },
        )?;
        let feedback_message_id = input
            .feedback
            .map(|feedback| {
                self.publish_feedback(
                    &input.scope_id,
                    kind,
                    &record_id,
                    &input.actor,
                    input.declared_by.as_deref(),
                    &request_id,
                    feedback,
                )
            })
            .transpose()?;
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
            fact: CycleFact::Decided,
            disposition_id: Some(disposition_id),
            feedback_message_id,
            actor: input.actor.id,
            request_id,
            intent_digest: digest,
        };
        write_receipt(self, &entry)?;
        Ok(entry)
    }

    #[rule("rule_rejection_comment_is_optional")]
    #[allow(clippy::too_many_arguments)]
    fn publish_feedback(
        &self,
        scope: &provenance_core::ScopeId,
        kind: NodeType,
        record_id: &StableId,
        actor: &provenance_core::DispositionActor,
        declared_by: Option<&str>,
        request_id: &StableId,
        feedback: ReviewFeedback,
    ) -> anyhow::Result<StableId> {
        use crate::review::discussion_input::{DiscussionAction, WriteDiscussion};

        let discussion = WriteDiscussion {
            scope_id: scope.clone(),
            parent: ThreadParent {
                node_type: kind,
                node_id: record_id.clone(),
            },
            request_id: feedback_request_id(request_id)?,
            actor: actor.id.clone(),
            declared_by: declared_by.map(str::to_string),
            action: DiscussionAction::Start {
                role: feedback.role,
                body: feedback.body,
            },
        };
        let digest = crate::review::discussion_writes::intent(&discussion)?;
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
}
