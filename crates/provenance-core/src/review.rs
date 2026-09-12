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
    Created,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<crate::threads::DiscussionOrigin>,
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

/// R1 Requirement entries keep their original representation in the shared journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JournalEntry {
    Requirement(Box<ReviewEntry>),
    Discussion(Box<crate::threads::DiscussionEntry>),
    Cycle(Box<CycleEntry>),
}
impl JournalEntry {
    pub const fn id(&self) -> &StableId {
        match self {
            Self::Requirement(e) => &e.id,
            Self::Discussion(e) => &e.id,
            Self::Cycle(e) => &e.id,
        }
    }
    pub const fn scope_id(&self) -> &ScopeId {
        match self {
            Self::Requirement(e) => &e.scope_id,
            Self::Discussion(e) => &e.scope_id,
            Self::Cycle(e) => &e.scope_id,
        }
    }
    pub const fn request_id(&self) -> &StableId {
        match self {
            Self::Requirement(e) => &e.request_id,
            Self::Discussion(e) => &e.request_id,
            Self::Cycle(e) => &e.request_id,
        }
    }
}

/// One decision-cycle fact: a submission, a decision, or a withdrawal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CycleFact {
    Submitted,
    Decided,
    Withdrawn,
}

/// One committed decision-cycle fact is also its durable request receipt.
///
/// The cycle never mutates a proposal, a disposition, or the record itself.
/// Submission writes an immutable bound proposal, decision writes an immutable
/// disposition, and this entry records which of the three happened, so the
/// request can be replayed and the history read in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CycleEntry {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub requirement_id: StableId,
    pub proposal_id: StableId,
    /// Submission and decision order for the addressed record.
    pub sequence: u64,
    pub fact: CycleFact,
    /// The disposition a `Decided` fact recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition_id: Option<StableId>,
    /// The feedback Message a decision published with its disposition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_message_id: Option<StableId>,
    pub actor: String,
    pub request_id: StableId,
    pub intent_digest: String,
}

/// A submission still waiting for a decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingSubmission {
    pub proposal_id: StableId,
    pub revision: StableId,
    pub actor: String,
    pub revises: Option<StableId>,
}

/// One terminal decision as the cycle reads it: the disposition itself, the
/// exact revision it decided on when its proposal is bound, and the feedback
/// Message published with it, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedDecision {
    pub disposition: crate::DispositionRecord,
    /// `None` for a legacy unbound disposition, which stays authoritative for
    /// its own proposal without attesting the record's current contents.
    pub revision: Option<StableId>,
    pub feedback_message_id: Option<StableId>,
}

/// The current and historical decision state of one record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementDecisionState {
    pub requirement_id: StableId,
    pub current_revision: Option<StableId>,
    pub pending: Option<PendingSubmission>,
    /// The accepted decision whose revision matches current content. Editing
    /// keeps past acceptances in `decisions` and leaves this empty.
    pub current_acceptance: Option<RecordedDecision>,
    /// Every terminal decision for the record, oldest first. Legacy unbound
    /// dispositions keep their place here.
    pub decisions: Vec<RecordedDecision>,
    /// Proposals withdrawn from review, oldest first. Withdrawal preserves the
    /// candidate, its feedback, and the graph record.
    pub withdrawn: Vec<StableId>,
}
