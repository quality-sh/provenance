//! Native review evidence types. Review does not change record lifecycle.
use crate::SchemaVersion;

mod review_serde;

pub const REVIEW_SCHEMA_VERSION: SchemaVersion = SchemaVersion(3);

use crate::{NodeType, ScopeId, StableId};
use serde::{Deserialize, Serialize};

const fn requirement_kind() -> NodeType {
    NodeType::Requirement
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_requirement_kind(kind: &NodeType) -> bool {
    matches!(kind, NodeType::Requirement)
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveOutcome {
    Created,
    Changed,
    LifecycleOnly,
}

/// Identifies one record type that native review supports.
pub trait ReviewRecordKind {
    const KIND: NodeType;

    fn review_id(&self) -> &StableId;
}

/// The closed list of record kinds that native review evidence supports.
///
/// To add a kind, define its record type and add one entry here. This list
/// generates the review enum, record dispatch, serialization, and closed
/// deserialization.
macro_rules! review_record_kinds {
    ($consumer:path) => {
        $consumer! {
            Source(crate::Source, Source),
            Requirement(crate::Requirement, Requirement),
            Resolution(crate::Resolution, Resolution),
            Rule(crate::Rule, Rule),
            Domain(crate::Domain, Domain),
            Boundary(crate::Boundary, Boundary),
            Topic(crate::Topic, Topic),
            Question(crate::Question, Question),
        }
    };
}
pub(crate) use review_record_kinds;

macro_rules! define_review_record_kinds {
    ($( $variant:ident($record:ty, $kind:ident), )*) => {
        pub const REVIEW_RECORD_KINDS: &[NodeType] = &[
            $(NodeType::$kind,)*
        ];
    };
}

review_record_kinds!(define_review_record_kinds);

macro_rules! define_review_record {
    ($( $variant:ident($record:ty, $kind:ident), )*) => {
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[derive(Debug, Clone)]
        pub enum ReviewRecord {
            $( $variant($record), )*
        }

        impl ReviewRecord {
            pub const fn kind(&self) -> NodeType {
                match self {
                    $( Self::$variant(_) => NodeType::$kind, )*
                }
            }

            pub const fn scope_id(&self) -> &ScopeId {
                match self {
                    $( Self::$variant(record) => &record.scope_id, )*
                }
            }

            pub const fn id(&self) -> &StableId {
                match self {
                    $( Self::$variant(record) => &record.id, )*
                }
            }

            pub const fn as_requirement(&self) -> Option<&crate::Requirement> {
                match self {
                    Self::Requirement(record) => Some(record),
                    _ => None,
                }
            }
        }

        $(
            impl From<$record> for ReviewRecord {
                fn from(record: $record) -> Self {
                    Self::$variant(record)
                }
            }

            impl ReviewRecordKind for $record {
                const KIND: NodeType = NodeType::$kind;

                fn review_id(&self) -> &StableId {
                    &self.id
                }
            }
        )*
    };
}

review_record_kinds!(define_review_record);

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementEditState {
    pub etag: String,
    pub revision: Option<StableId>,
}

/// One version of a record: a commit that changed the record in its graph
/// record file, or the saved working copy when it differs from `HEAD`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordVersion {
    /// The commit id, or `working` for the saved working copy.
    pub id: StableId,
    pub commit: Option<String>,
    pub author: Option<String>,
    /// Commit time in seconds since the Unix epoch.
    pub committed_at: Option<i64>,
    pub revision: StableId,
    /// The id of the previous version, if any.
    pub before: Option<StableId>,
    pub changed_fields: Vec<String>,
    pub outcome: SaveOutcome,
    /// The Discussion outcome first recorded with this version.
    pub origin: Option<crate::threads::DiscussionOrigin>,
}

/// JSON text spans concatenate to the canonical JSON of one record version,
/// or of one field of it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidencePage {
    pub version: StableId,
    pub offset: u64,
    pub field: Option<String>,
    pub json_text: String,
    pub next_offset: Option<u64>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewHistoryQuery {
    #[serde(
        default = "requirement_kind",
        skip_serializing_if = "is_requirement_kind"
    )]
    pub record_kind: NodeType,
    #[serde(rename = "requirement_id")]
    pub record_id: StableId,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}
const fn default_limit() -> usize {
    50
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewHistoryPage {
    pub entries: Vec<RecordVersion>,
    pub next_cursor: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceQuery {
    #[serde(
        default = "requirement_kind",
        skip_serializing_if = "is_requirement_kind"
    )]
    pub record_kind: NodeType,
    #[serde(rename = "requirement_id")]
    pub record_id: StableId,
    pub entry_id: StableId,
    pub before: bool,
    pub field: Option<String>,
    #[serde(default)]
    pub offset: u64,
}

/// One decision-cycle fact: a submission, a decision, or a withdrawal.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CycleFact {
    Submitted,
    Decided,
    Withdrawn,
}

/// The response to one decision-cycle write.
///
/// The cycle never mutates a proposal, a disposition, or the record itself.
/// Submission writes an immutable bound Proposal, decision writes an immutable
/// Disposition, and withdrawal writes a Withdrawal. This receipt names the
/// record that the write added and is not stored.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CycleEntry {
    pub scope_id: ScopeId,
    pub id: StableId,
    #[serde(
        default = "requirement_kind",
        skip_serializing_if = "is_requirement_kind"
    )]
    pub record_kind: NodeType,
    #[serde(rename = "requirement_id")]
    pub record_id: StableId,
    pub proposal_id: StableId,
    /// The server-created key of a `Submitted` Proposal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_key: Option<String>,
    /// The review cycle of the submission for the addressed record.
    pub sequence: u64,
    pub fact: CycleFact,
    /// The disposition a `Decided` fact recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition_id: Option<StableId>,
    /// The feedback Message a decision published with its disposition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_message_id: Option<StableId>,
    pub actor: String,
}

/// A submission still waiting for a decision.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementDecisionState {
    #[serde(
        default = "requirement_kind",
        skip_serializing_if = "is_requirement_kind"
    )]
    pub record_kind: NodeType,
    #[serde(rename = "requirement_id")]
    pub record_id: StableId,
    pub current_revision: Option<StableId>,
    pub pending: Option<PendingSubmission>,
    /// The accepted decision whose revision matches current content. An edit
    /// to other content leaves this empty and keeps past acceptances in
    /// `decisions`.
    pub current_acceptance: Option<RecordedDecision>,
    /// Every terminal decision for the record, oldest first. Legacy unbound
    /// dispositions keep their place here.
    pub decisions: Vec<RecordedDecision>,
    /// Proposals withdrawn from review, oldest first. Withdrawal preserves the
    /// candidate, its feedback, and the graph record.
    pub withdrawn: Vec<StableId>,
}

#[cfg(test)]
mod tests;
