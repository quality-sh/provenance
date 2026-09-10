use super::GraphNode;
use crate::{Message, StableId, Thread};
use serde::{Deserialize, Serialize};

/// Selects one active Requirement branch at a projection revision.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadDocumentQuery {
    pub id: String,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default = "default_limit")]
    #[cfg_attr(feature = "schema", schemars(range(min = 1, max = super::QUERY_MAX_LIMIT)))]
    pub limit: usize,
}
const fn default_limit() -> usize {
    super::QUERY_DEFAULT_LIMIT
}

/// A record's role in this document. References never expand membership.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentEntry {
    Member { node: GraphNode },
    Reference { node: GraphNode },
    Thread { thread: Thread },
    Message { message: Message },
}

/// One bounded page. Only a terminal sequence establishes completeness.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize)]
pub struct ReadDocumentResult {
    pub root_id: StableId,
    pub limit: usize,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub entries: Vec<DocumentEntry>,
}
