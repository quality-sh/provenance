use super::{DiscussionAction, WriteDiscussion};
use crate::{
    state_store::StateStore,
    write_error::{SourceFailure, WriteFailure},
};
use provenance_core::{threads::Discussion, MessageRole, NodeType, ScopeId, StableId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TargetDiscussionWrite {
    pub scope_id: ScopeId,
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
    ) -> anyhow::Result<Discussion> {
        self.with_repository_publication(|| {
            let head = self
                .validated_discussions(&input.scope_id)?
                .into_iter()
                .find(|discussion| discussion.discussion_id == input.discussion_id)
                .ok_or_else(|| {
                    SourceFailure::wrap(
                        WriteFailure::ResourceNotFound,
                        anyhow::anyhow!("Discussion does not exist"),
                    )
                })?;
            let parent = head.parent.clone();
            crate::write_error::ensure!(
                ResourceNotFound,
                input.allowed_parent_kinds.contains(&parent.node_type),
                "Discussion parent is not available"
            );
            self.write_discussion_in_publication(
                WriteDiscussion {
                    scope_id: input.scope_id,
                    parent,
                    actor: input.actor,
                    declared_by: input.declared_by,
                    action: DiscussionAction::Reply {
                        discussion_id: input.discussion_id,
                        expected_version: input.expected_version,
                        role: input.role,
                        body: input.body,
                    },
                },
                Some(head),
            )
        })
    }
}
