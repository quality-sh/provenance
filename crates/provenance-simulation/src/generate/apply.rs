//! Each operation of the generator as a call of the store.

use super::{
    binding_file, created_requirement_id, requirement_id, Effect, Operation, Task,
    ARCHIVE_COMMIT, DECIDER, FIRST_REQUIREMENT, FIRST_RULE, TOPIC,
};
use crate::{scope, stable};
use provenance_core::{MessageRole, NodeType, RequirementStatus, ThreadParent};
use provenance_store::repository::Repository;
use provenance_store::state_store::{
    CreateRequirementInput, MaterializeVerificationBindingInput, PostMessageInput,
    UpdateRequirementInput,
};

/// The Thread parent of messages and Discussions: the first Requirement.
fn first_parent() -> ThreadParent {
    ThreadParent {
        node_type: NodeType::Requirement,
        node_id: stable(FIRST_REQUIREMENT),
    }
}

/// An update of one Requirement that changes only what `change` sets.
fn requirement_update(
    id: &str,
    change: impl FnOnce(&mut UpdateRequirementInput),
) -> UpdateRequirementInput {
    let mut update = UpdateRequirementInput {
        scope_id: scope(),
        id: stable(id),
        expected_etag: None,
        declared_by: None,
        statement: None,
        description: None,
        status: None,
        archived_in_commit: None,
        clear_fields: Vec::new(),
    };
    change(&mut update);
    update
}

/// The stamp of an archive.
pub fn archive_stamp() -> provenance_core::ArchivedStamp {
    provenance_core::ArchivedStamp {
        commit: ARCHIVE_COMMIT.into(),
        at: None,
    }
}

/// Runs one operation on `repository`.
// One arm per operation; splitting them would hide the list.
#[allow(clippy::too_many_lines)]
pub fn apply(repository: &Repository, task: &Task) -> anyhow::Result<Effect> {
    use provenance_store::review::{
        DecideRecordReview, DiscussionAction, SubmitRecordReview, WithdrawRecordReview,
        WriteDiscussion,
    };
    let store = repository.store();
    Ok(match &task.op {
        Operation::CreateRequirement { n } => {
            let id = created_requirement_id(*n);
            store.create_requirement(CreateRequirementInput {
                scope_id: scope(),
                id: stable(&id),
                statement: "Share links have an owner".into(),
                description: None,
                status: RequirementStatus::Active,
                refines: Vec::new(),
                depends_on: Vec::new(),
                supersedes: Vec::new(),
                spawned_by: None,
                origin_thread: None,
                origin_message: None,
            })?;
            Effect::Created { id }
        }
        Operation::UpdateRequirement { n, stale } => {
            let id = requirement_id(*n);
            let etag = if *stale {
                task.stale_etag.clone()
            } else {
                store.requirement_edit_state(&scope(), &stable(&id))?.etag
            };
            let description = task.description();
            store.update_requirement(requirement_update(&id, |update| {
                update.expected_etag = Some(etag.clone());
                update.description = Some(description.clone());
            }))?;
            Effect::Described {
                id,
                description,
                etag,
            }
        }
        Operation::PostMessage => {
            let posted = store.post_thread_message(PostMessageInput {
                scope_id: scope(),
                parent: first_parent(),
                role: MessageRole::User,
                body: task.body(),
            })?;
            Effect::Message {
                id: posted.message.id.as_str().to_owned(),
            }
        }
        Operation::SubmitReview => {
            let entry = store.submit_record_review(SubmitRecordReview {
                scope_id: scope(),
                actor: "reviewer".into(),
                record_kind: NodeType::Requirement,
                record_id: stable(FIRST_REQUIREMENT),
                declared_by: None,
                title: "Simulated review".into(),
                summary: format!("Review from operation {}", task.operation),
                confidence: None,
                source_ids: Vec::new(),
                evidence_references: Vec::new(),
                builds_on: Vec::new(),
                expected_revision: None,
                revises: None,
            })?;
            Effect::Proposal {
                id: entry.proposal_id.as_str().to_owned(),
            }
        }
        Operation::WithdrawReview { .. } => {
            let Some(proposal) = &task.proposal else {
                return Ok(Effect::Skipped);
            };
            store.withdraw_record_review(WithdrawRecordReview {
                scope_id: scope(),
                actor: "reviewer".into(),
                proposal_id: stable(proposal),
                declared_by: None,
                reason: None,
            })?;
            Effect::Withdrawn {
                proposal: proposal.clone(),
            }
        }
        Operation::DecideReview { .. } => {
            let Some(proposal) = &task.proposal else {
                return Ok(Effect::Skipped);
            };
            store.decide_record_review(DecideRecordReview {
                scope_id: scope(),
                actor: decider(),
                proposal_id: stable(proposal),
                decision: provenance_core::DispositionDecision::Rejected,
                rationale: Some("The simulation rejects it".into()),
                canonical_artifact: None,
                feedback: None,
                declared_by: None,
            })?;
            Effect::Decided {
                proposal: proposal.clone(),
            }
        }
        Operation::WriteDiscussion { .. } => {
            let action = match &task.discussion {
                Some((id, version)) => DiscussionAction::Reply {
                    discussion_id: stable(id),
                    expected_version: *version,
                    role: MessageRole::User,
                    body: task.body(),
                },
                None => DiscussionAction::Start {
                    role: MessageRole::User,
                    body: task.body(),
                },
            };
            let discussion = store.write_discussion(WriteDiscussion {
                scope_id: scope(),
                parent: first_parent(),
                actor: task.actor(),
                declared_by: None,
                action,
            })?;
            Effect::Discussion {
                id: discussion.discussion_id.as_str().to_owned(),
                version: discussion.version,
            }
        }
        Operation::ArchiveRequirement { n } => {
            let id = requirement_id(*n);
            store.update_requirement(requirement_update(&id, |update| {
                update.status = Some(RequirementStatus::Archived);
                update.archived_in_commit = Some(archive_stamp());
            }))?;
            Effect::Archived { id }
        }
        Operation::ClaimTopic => {
            store.claim_topic(&scope(), &stable(TOPIC), &task.actor())?;
            Effect::Claim {
                holder: Some(task.actor()),
            }
        }
        Operation::ReleaseTopic => {
            store.release_topic(&scope(), &stable(TOPIC), &task.actor())?;
            Effect::Claim { holder: None }
        }
        Operation::BindTwice { first, second } => {
            let binding = |n: u8| MaterializeVerificationBindingInput {
                scope_id: scope(),
                rule_id: stable(FIRST_RULE),
                key: "simulated-binding".into(),
                method: provenance_core::VerificationMethod::Examples,
                declared_by: "test://simulation".into(),
                file: binding_file(n).into(),
                symbol: None,
            };
            let (first, second) = store.with_repository_publication(|section| {
                let first = section.materialize_verification_binding(binding(*first));
                let second = section.materialize_verification_binding(binding(*second));
                Ok((first.is_ok(), second.is_ok()))
            })?;
            Effect::Bound { first, second }
        }
    })
}

/// The actor that decides a review.
pub fn decider() -> provenance_core::DispositionActor {
    provenance_core::DispositionActor {
        identity_type: provenance_core::IdentityType::Human,
        id: DECIDER.into(),
        name: None,
    }
}
