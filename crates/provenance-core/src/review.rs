//! Native review evidence types. Review does not change record lifecycle.
use crate::SchemaVersion;

pub const REVIEW_SCHEMA_VERSION: SchemaVersion = SchemaVersion(3);

use crate::{Requirement, ScopeId, StableId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotRef {
    pub id: StableId,
    pub digest: String,
    pub bytes: u64,
    pub fields: Vec<SnapshotField>,
}

/// A field range in an ordinary JSON snapshot. Offsets count UTF-8 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotField {
    pub name: String,
    pub offset: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveOutcome {
    Enrolled,
    Changed,
    LifecycleOnly,
    NoChange,
}

/// One committed save is also its durable request receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewEntry {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub requirement_id: StableId,
    pub id: StableId,
    pub sequence: u64,
    pub predecessor: Option<StableId>,
    pub revision: StableId,
    pub prior_revision: Option<StableId>,
    pub before: Option<SnapshotRef>,
    pub after: SnapshotRef,
    pub changed_fields: Vec<String>,
    pub actor: String,
    pub request_id: StableId,
    pub intent_digest: String,
    pub etag: String,
    pub outcome: SaveOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementSnapshot {
    pub schema_version: SchemaVersion,
    pub record: Requirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementEditState {
    pub etag: String,
    pub revision: Option<StableId>,
    pub snapshot: Option<SnapshotRef>,
}

/// JSON text spans concatenate to the exact immutable snapshot document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePage {
    pub snapshot: SnapshotRef,
    pub offset: u64,
    pub field: Option<String>,
    pub json_text: String,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewHistoryQuery {
    pub requirement_id: StableId,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}
const fn default_limit() -> usize {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewHistoryPage {
    pub entries: Vec<ReviewEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceQuery {
    pub requirement_id: StableId,
    pub entry_id: StableId,
    pub before: bool,
    pub field: Option<String>,
    #[serde(default)]
    pub offset: u64,
}
