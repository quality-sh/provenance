use super::validate_submission_address;
use crate::{
    publication::with_staged_state,
    review::{
        classifier,
        decision_input::{DecideRecordReview, ReviewFeedback},
        decision_state::{request_digest, review_submission, write_receipt, CycleFacts},
        guard, journal,
    },
    shards,
    state_store::{CreateDispositionInput, StateStore},
    write_error::{SourceFailure, WriteFailure},
};
use provenance_core::{
    review::{CycleEntry, CycleFact, REVIEW_SCHEMA_VERSION},
    CanonicalArtifactType, DispositionDecision, NodeType, StableId, ThreadParent,
};
use provenance_macros::rule;

impl StateStore {
    #[rule("rule_approval_accepts_reviewed_version")]
    pub fn decide_record_review(&self, input: DecideRecordReview) -> anyhow::Result<CycleEntry> {
        self.decide_record_review_addressed(None, input)
    }

    pub fn decide_record_review_for(
        &self,
        kind: NodeType,
        record_id: &StableId,
        input: DecideRecordReview,
    ) -> anyhow::Result<CycleEntry> {
        self.decide_record_review_addressed(Some((kind, record_id.clone())), input)
    }

    /// Rejects feedback for record kinds that cannot carry a Discussion.
    #[rule("rule_domain_boundary_decisions_accept_no_feedback")]
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
            validate_decision_input(kind, record_id, &input)?;
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

    /// Creates the server-owned Disposition identity for a review decision.
    #[rule("rule_review_disposition_identity_server_created")]
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
        let record =
            crate::cache::review_families::record(self, &input.scope_id, kind, &record_id)?;
        let revision = super::super::save::current_revision(&record)?
            .ok_or_else(|| anyhow::anyhow!("the submitted record has no review revision"))?;
        if revision != binding.revision
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

    #[allow(clippy::too_many_arguments)]
    fn publish_feedback(
        &self,
        scope: &provenance_core::ScopeId,
        kind: NodeType,
        record_id: &StableId,
        actor: &provenance_core::DispositionActor,
        declared_by: Option<&str>,
        feedback: ReviewFeedback,
    ) -> anyhow::Result<StableId> {
        use crate::review::discussion_input::{DiscussionAction, WriteDiscussion};

        let discussion = WriteDiscussion {
            scope_id: scope.clone(),
            parent: ThreadParent {
                node_type: kind,
                node_id: record_id.clone(),
            },
            actor: actor.id.clone(),
            declared_by: declared_by.map(str::to_string),
            action: DiscussionAction::Start {
                role: feedback.role,
                body: feedback.body,
            },
        };
        crate::review::discussion_writes::check_size(&discussion)?;
        self.authorize_discussion(&discussion)?;
        let started = guard::with_writer(&shards::threads_path(&self.layout, scope), "*", || {
            guard::with_writer(&shards::messages_path(&self.layout, scope), "*", || {
                self.commit_discussion(discussion, None)
            })
        })?;
        Ok(started.root_message_id)
    }
}

/// Rejects rationale text on an approval before any decision is written.
#[rule("rule_review_approval_has_no_rationale")]
fn validate_decision_input(
    kind: NodeType,
    record_id: &StableId,
    input: &DecideRecordReview,
) -> anyhow::Result<()> {
    if input.decision == DispositionDecision::Accepted && input.rationale.is_some() {
        return Err(SourceFailure::wrap(
            WriteFailure::InvalidUpdate,
            anyhow::anyhow!("an approval does not take a rationale"),
        ));
    }
    if let Some(artifact) = &input.canonical_artifact {
        let artifact_kind = match artifact.artifact_type {
            CanonicalArtifactType::Source => NodeType::Source,
            CanonicalArtifactType::Requirement => NodeType::Requirement,
            CanonicalArtifactType::Resolution => NodeType::Resolution,
            CanonicalArtifactType::Rule => NodeType::Rule,
            CanonicalArtifactType::Domain => NodeType::Domain,
            CanonicalArtifactType::Boundary => NodeType::Boundary,
            CanonicalArtifactType::Topic => NodeType::Topic,
            CanonicalArtifactType::Question => NodeType::Question,
        };
        if artifact_kind != kind || artifact.artifact_id != *record_id {
            return Err(SourceFailure::wrap(
                WriteFailure::InvalidUpdate,
                anyhow::anyhow!("the approval artifact is not the reviewed record"),
            ));
        }
    }
    Ok(())
}
