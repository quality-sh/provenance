use crate::{NodeType, ScopeId, StableId, ThreadParent};
use provenance_macros::ProjectionRow;
use serde::{Deserialize, Serialize};

/// A Discussion's status is independent of its Thread container.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscussionStatus {
    Active,
    Resolved,
}

/// One Discussion: its root Message, the Messages that belong to it, its
/// status, and the records created or changed from it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ProjectionRow)]
#[serde(deny_unknown_fields)]
#[table("discussions")]
pub struct Discussion {
    pub scope_id: ScopeId,
    pub discussion_id: StableId,
    #[column(json)]
    pub parent: ThreadParent,
    pub thread_id: StableId,
    pub root_message_id: StableId,
    /// The Messages of the Discussion in write order. The first is the root.
    pub message_ids: Vec<StableId>,
    pub status: DiscussionStatus,
    /// Increases by one at each reply or status change.
    pub version: u64,
    /// The actor who started the Discussion.
    pub actor: String,
    pub outcomes: Vec<DiscussionOutcome>,
    /// The review decision that published this Discussion as its feedback.
    pub disposition_id: Option<StableId>,
}

impl Discussion {
    pub const fn id(&self) -> &StableId {
        &self.discussion_id
    }

    /// Refuses a Discussion whose root, versions, or membership disagree.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.actor.trim().is_empty(), "invalid Discussion actor");
        anyhow::ensure!(
            self.message_ids.first() == Some(&self.root_message_id),
            "Discussion has no root Message"
        );
        let mut ids = std::collections::BTreeSet::new();
        anyhow::ensure!(
            self.message_ids.iter().all(|id| ids.insert(id.as_str())),
            "Discussion lists a Message twice"
        );
        anyhow::ensure!(
            self.version >= 1 && self.version >= self.message_ids.len() as u64,
            "Discussion version is lower than its writes"
        );
        anyhow::ensure!(
            self.outcomes
                .iter()
                .all(|outcome| ids.contains(outcome.message_id.as_str())),
            "Discussion outcome cites a Message outside the Discussion"
        );
        Ok(())
    }
}

/// A record created or changed from one Message of a Discussion, with the
/// review revision that the write gave the record.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionOutcome {
    pub message_id: StableId,
    pub record_kind: NodeType,
    pub record_id: StableId,
    pub revision: StableId,
}

/// An outcome cites one known Message in one Discussion. It does not infer membership.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionOrigin {
    pub thread_id: StableId,
    pub discussion_id: StableId,
    pub message_id: StableId,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiscussionGroup {
    Addressed {
        discussion: Box<Discussion>,
        container_status: crate::ThreadStatus,
    },
    Legacy {
        thread_id: StableId,
        parent: ThreadParent,
        container_status: crate::ThreadStatus,
    },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionQuery {
    pub parent: ThreadParent,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionPage {
    pub entries: Vec<DiscussionGroup>,
    pub next_cursor: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiscussionSelector {
    Discussion { discussion_id: StableId },
    Legacy { thread_id: StableId },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionMessagesQuery {
    pub parent: ThreadParent,
    pub selector: DiscussionSelector,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionMessagesPage {
    pub entries: Vec<crate::Message>,
    pub next_cursor: Option<String>,
}

const fn default_limit() -> usize {
    50
}
