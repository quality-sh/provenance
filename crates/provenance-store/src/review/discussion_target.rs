use super::{DiscussionAction, WriteDiscussion};
use crate::{state_store::StateStore, write_error::{SourceFailure, WriteFailure}};
use provenance_core::{threads::DiscussionEntry, MessageRole, ScopeId, StableId, ThreadParent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TargetDiscussionWrite {
    pub scope_id: ScopeId,
    pub request_id: StableId,
    pub actor: String,
    pub declared_by: Option<String>,
    pub action: TargetDiscussionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetDiscussionAction {
    Start {
        parent: ThreadParent,
        role: MessageRole,
        body: String,
    },
    Reply {
        discussion_id: StableId,
        expected_version: u64,
        role: MessageRole,
        body: String,
    },
}

impl StateStore {
    /// Resolves the target and writes its Discussion in one publication.
    pub fn write_target_discussion(
        &self,
        input: TargetDiscussionWrite,
    ) -> anyhow::Result<DiscussionEntry> {
        self.with_repository_publication(|| {
            let (parent, action) = match input.action {
                TargetDiscussionAction::Start { parent, role, body } => {
                    (parent, DiscussionAction::Start { role, body })
                }
                TargetDiscussionAction::Reply {
                    discussion_id,
                    expected_version,
                    role,
                    body,
                } => {
                    let head = self
                        .discussion_heads(&input.scope_id)?
                        .into_iter()
                        .find(|entry| entry.discussion_id == discussion_id)
                        .ok_or_else(|| SourceFailure::wrap(
                            WriteFailure::ResourceNotFound,
                            anyhow::anyhow!("Discussion does not exist"),
                        ))?;
                    (head.parent, DiscussionAction::Reply {
                        discussion_id,
                        expected_version,
                        role,
                        body,
                    })
                }
            };
            self.write_discussion(WriteDiscussion {
                scope_id: input.scope_id,
                parent,
                request_id: input.request_id,
                actor: input.actor,
                declared_by: input.declared_by,
                action,
            })
        })
    }
}
