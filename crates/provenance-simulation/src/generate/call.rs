//! Each operation of the generator as a call of the public contract.

use super::{
    created_requirement_id, requirement_id, Effect, Operation, Task, ARCHIVE_COMMIT, DECIDER,
    FIRST_REQUIREMENT, SCOPE, TOPIC,
};
use crate::{scope, stable};
use provenance_store::repository::Repository;
use serde::Serialize;
use serde_json::{json, Value};

/// One operation as a call of the public contract: its name and request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Call {
    pub operation: &'static str,
    pub request: Value,
}

/// The parent of messages and Discussions in a request.
fn first_parent() -> Value {
    json!({"node_type": "requirement", "node_id": FIRST_REQUIREMENT})
}

impl Task {
    /// The current etag of Requirement `id`, or the stale etag.
    fn etag(&self, repository: &Repository, id: &str, stale: bool) -> anyhow::Result<String> {
        if stale {
            return Ok(self.stale_etag.clone());
        }
        let store = repository.store();
        Ok(store.requirement_edit_state(&scope(), &stable(id))?.etag)
    }

    /// The contract call of this operation, or `None` when the contract has
    /// no operation for it (two binding writes in one section) or when there
    /// is nothing to act on. An update without `stale` reads the current
    /// etag from `repository` first, as the engine does.
    pub fn call(&self, repository: &Repository) -> anyhow::Result<Option<Call>> {
        let call = |operation, request| Some(Call { operation, request });
        let review = |operation, proposal: &String, extra: Value| {
            let mut request = json!({
                "scope_id": SCOPE, "record_kind": "requirement",
                "record_id": FIRST_REQUIREMENT, "proposal_id": proposal,
            });
            if let (Some(request), Value::Object(extra)) = (request.as_object_mut(), extra) {
                request.extend(extra);
            }
            call(operation, request)
        };
        Ok(match &self.op {
            Operation::CreateRequirement { n } => call(
                "create-requirement-native",
                json!({
                    "scope_id": SCOPE, "id": created_requirement_id(*n),
                    "statement": "Share links have an owner", "status": "active",
                    "refines": [], "depends_on": [], "supersedes": [],
                }),
            ),
            Operation::UpdateRequirement { n, stale } => {
                let id = requirement_id(*n);
                call(
                    "update-requirement",
                    json!({
                        "id": id, "actor": self.actor(),
                        "expected_etag": self.etag(repository, &id, *stale)?,
                        "description": self.description(), "clear_fields": [],
                    }),
                )
            }
            Operation::ArchiveRequirement { n } => {
                let id = requirement_id(*n);
                call(
                    "update-requirement",
                    json!({
                        "id": id, "actor": self.actor(),
                        "expected_etag": self.etag(repository, &id, false)?,
                        "status": "archived", "archived_in_commit": {"commit": ARCHIVE_COMMIT},
                        "clear_fields": [],
                    }),
                )
            }
            Operation::PostMessage => call(
                "post-thread-message",
                json!({
                    "scope_id": SCOPE, "parent": first_parent(), "role": "user",
                    "body": self.body(),
                }),
            ),
            Operation::SubmitReview => call(
                "submit-record-review",
                json!({
                    "scope_id": SCOPE, "actor": "reviewer", "record_kind": "requirement",
                    "record_id": FIRST_REQUIREMENT, "title": "Simulated review",
                    "summary": format!("Review from operation {}", self.operation),
                    "source_ids": [], "evidence_references": [], "builds_on": [],
                }),
            ),
            Operation::WithdrawReview { .. } => self.proposal.as_ref().and_then(|proposal| {
                review(
                    "withdraw-record-review",
                    proposal,
                    json!({"actor": "reviewer"}),
                )
            }),
            Operation::DecideReview { .. } => self.proposal.as_ref().and_then(|proposal| {
                review(
                    "decide-record-review",
                    proposal,
                    json!({
                        "actor": {"identity_type": "human", "id": DECIDER},
                        "decision": "rejected", "rationale": "The simulation rejects it",
                    }),
                )
            }),
            Operation::WriteDiscussion { .. } => {
                let action = match &self.discussion {
                    Some((id, version)) => json!({
                        "kind": "reply", "discussion_id": id, "expected_version": version,
                        "role": "user", "body": self.body(),
                    }),
                    None => json!({"kind": "start", "role": "user", "body": self.body()}),
                };
                call(
                    "write-discussion",
                    json!({
                        "scope_id": SCOPE, "parent": first_parent(), "actor": self.actor(),
                        "action": action,
                    }),
                )
            }
            Operation::ClaimTopic => call(
                "claim-topic",
                json!({"scope_id": SCOPE, "id": TOPIC, "actor": self.actor()}),
            ),
            Operation::ReleaseTopic => call(
                "release-topic",
                json!({"scope_id": SCOPE, "id": TOPIC, "actor": self.actor()}),
            ),
            Operation::BindTwice { .. } => None,
        })
    }

    /// What a successful contract `call` of this operation did, from its
    /// `answer`.
    pub fn effect(&self, call: &Call, answer: &Value) -> anyhow::Result<Effect> {
        let text = |value: &Value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                anyhow::anyhow!("{} answered without an ID: {answer}", call.operation)
            })
        };
        Ok(match &self.op {
            Operation::CreateRequirement { n } => Effect::Created {
                id: created_requirement_id(*n),
            },
            Operation::UpdateRequirement { n, .. } => Effect::Described {
                id: requirement_id(*n),
                description: self.description(),
                etag: text(&call.request["expected_etag"])?,
            },
            Operation::ArchiveRequirement { n } => Effect::Archived {
                id: requirement_id(*n),
            },
            Operation::PostMessage => Effect::Message {
                id: text(&answer["message"]["id"])?,
            },
            Operation::SubmitReview => Effect::Proposal {
                id: text(&answer["proposal_id"])?,
            },
            Operation::WithdrawReview { .. } => Effect::Withdrawn {
                proposal: text(&call.request["proposal_id"])?,
            },
            Operation::DecideReview { .. } => Effect::Decided {
                proposal: text(&call.request["proposal_id"])?,
            },
            Operation::WriteDiscussion { .. } => Effect::Discussion {
                id: text(&answer["discussion_id"])?,
                version: answer["version"].as_u64().ok_or_else(|| {
                    anyhow::anyhow!("{} answered without a version: {answer}", call.operation)
                })?,
            },
            Operation::ClaimTopic => Effect::Claim {
                holder: Some(self.actor()),
            },
            Operation::ReleaseTopic => Effect::Claim { holder: None },
            Operation::BindTwice { .. } => {
                anyhow::bail!("the contract has no call for two bindings")
            }
        })
    }
}
