use super::{DiscussionEntry, DiscussionMessagesPage, DiscussionStatus};
use crate::{Message, NodeType, StableId, ThreadParent};
use provenance_macros::rule;
use serde::{Deserialize, Serialize};

/// The status selection for an addressed Discussion list.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[rule("rule_porcelain_discussions_default_active")]
pub enum DiscussionStatusFilter {
    #[default]
    Active,
    Resolved,
    All,
}

impl DiscussionStatusFilter {
    pub const ALL: [Self; 3] = [Self::Active, Self::Resolved, Self::All];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Resolved => "resolved",
            Self::All => "all",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.as_str() == word)
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionListQuery {
    pub parent: Option<ThreadParent>,
    pub allowed_parent_kinds: Vec<NodeType>,
    #[serde(default)]
    pub status: DiscussionStatusFilter,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionSummary {
    pub discussion_id: StableId,
    pub parent: ThreadParent,
    pub status: DiscussionStatus,
    pub version: u64,
    pub opening_excerpt: String,
    pub excerpt_truncated: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionListPage {
    pub entries: Vec<DiscussionSummary>,
    pub next_cursor: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscussionConversationQuery {
    pub discussion_id: StableId,
    pub allowed_parent_kinds: Vec<NodeType>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionConversation {
    pub head: DiscussionEntry,
    pub messages: DiscussionMessagesPage,
}

/// The page returned by a registered Discussion read operation.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionResultPage<T> {
    pub entries: Vec<T>,
    pub limit: usize,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

/// One Discussion head and a bounded Message page from one read snapshot.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscussionConversationResult {
    pub head: DiscussionEntry,
    pub messages: DiscussionResultPage<Message>,
}

const fn default_limit() -> usize {
    50
}
