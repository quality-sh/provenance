use crate::{SchemaVersion, ScopeId, StableId, ThreadParent};
use serde::{Deserialize, Serialize};

/// A Discussion's status is independent of its Thread container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscussionStatus {
    Active,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscussionFact {
    Started,
    Replied,
    StatusChanged,
}

/// One immutable membership or status fact is also the request receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionEntry {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub parent: ThreadParent,
    pub thread_id: StableId,
    pub discussion_id: StableId,
    pub root_message_id: StableId,
    pub version: u64,
    pub predecessor: Option<StableId>,
    pub status: DiscussionStatus,
    pub fact: DiscussionFact,
    pub message_id: Option<StableId>,
    pub actor: String,
    pub request_id: StableId,
    pub intent_digest: String,
}

/// An outcome cites one known Message in one Discussion. It does not infer membership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionOrigin {
    pub thread_id: StableId,
    pub discussion_id: StableId,
    pub message_id: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiscussionGroup {
    Addressed {
        discussion: Box<DiscussionEntry>,
        container_status: crate::ThreadStatus,
    },
    Legacy {
        thread_id: StableId,
        parent: ThreadParent,
        container_status: crate::ThreadStatus,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionQuery {
    pub parent: ThreadParent,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionPage {
    pub entries: Vec<DiscussionGroup>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiscussionSelector {
    Discussion { discussion_id: StableId },
    Legacy { thread_id: StableId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionMessagesQuery {
    pub parent: ThreadParent,
    pub selector: DiscussionSelector,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionMessagesPage {
    pub entries: Vec<crate::Message>,
    pub next_cursor: Option<String>,
}

const fn default_limit() -> usize {
    50
}
