use super::{journal, DiscussionAction, WriteDiscussion};
use crate::{
    state_store::StateStore,
    write_error::{SourceFailure, WriteFailure},
};
use provenance_core::{
    review::JournalEntry, threads::DiscussionEntry, MessageRole, NodeType, ScopeId, StableId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TargetDiscussionWrite {
    pub scope_id: ScopeId,
    pub request_id: StableId,
    pub actor: String,
    pub declared_by: Option<String>,
    /// Parent kinds granted by the host before this operation runs.
    pub allowed_parent_kinds: Vec<NodeType>,
    pub discussion_id: StableId,
    pub expected_version: u64,
    pub role: MessageRole,
    pub body: String,
}

impl StateStore {
    /// Resolves a reply target and writes it in one publication.
    pub fn write_target_discussion(
        &self,
        input: TargetDiscussionWrite,
    ) -> anyhow::Result<DiscussionEntry> {
        self.with_repository_publication(|| {
            let receipt_path =
                journal::entry_path(&self.layout, &input.scope_id, &input.request_id);
            let receipt_parent = if receipt_path.try_exists()? {
                match journal::read_journal_entry(&self.layout, &receipt_path)? {
                    JournalEntry::Discussion(entry) => Some(entry.parent),
                    _ => {
                        return Err(SourceFailure::wrap(
                            WriteFailure::DiscussionIntentChanged,
                            anyhow::anyhow!("request ID belongs to another write"),
                        ));
                    }
                }
            } else {
                None
            };
            let (parent, head) = if let Some(parent) = receipt_parent {
                (parent, None)
            } else {
                let head = self
                    .discussion_heads(&input.scope_id)?
                    .into_iter()
                    .find(|entry| entry.discussion_id == input.discussion_id)
                    .ok_or_else(|| {
                        SourceFailure::wrap(
                            WriteFailure::ResourceNotFound,
                            anyhow::anyhow!("Discussion does not exist"),
                        )
                    })?;
                (head.parent.clone(), Some(head))
            };
            crate::write_error::ensure!(
                ResourceNotFound,
                input.allowed_parent_kinds.contains(&parent.node_type),
                "Discussion parent is not available"
            );
            self.write_discussion_in_publication(
                WriteDiscussion {
                    scope_id: input.scope_id,
                    parent,
                    request_id: input.request_id,
                    actor: input.actor,
                    declared_by: input.declared_by,
                    action: DiscussionAction::Reply {
                        discussion_id: input.discussion_id,
                        expected_version: input.expected_version,
                        role: input.role,
                        body: input.body,
                    },
                },
                head,
            )
        })
    }
}
