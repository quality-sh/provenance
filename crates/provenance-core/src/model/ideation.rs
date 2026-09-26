use serde::{Deserialize, Serialize};

pub(super) mod contributions;
pub(super) mod dispositions;
pub(super) mod legacy_audit;
pub(super) mod lifecycle;
pub(super) mod proposals;
pub(super) mod synthesis;

use super::graph::NodeType;
use super::ids::StableId;
use super::parsing::parse_enum_word;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum IdeationTargetType {
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "requirement")]
    Requirement,
    #[serde(rename = "resolution")]
    Resolution,
    #[serde(rename = "rule")]
    Rule,
    #[serde(rename = "topic")]
    Topic,
    #[serde(rename = "question")]
    Question,
    #[serde(rename = "domain")]
    Domain,
    #[serde(rename = "boundary")]
    Boundary,
}

/// An ideation target names a graph record; this is the kind it names.
impl From<IdeationTargetType> for NodeType {
    fn from(target_type: IdeationTargetType) -> Self {
        match target_type {
            IdeationTargetType::Source => Self::Source,
            IdeationTargetType::Requirement => Self::Requirement,
            IdeationTargetType::Resolution => Self::Resolution,
            IdeationTargetType::Rule => Self::Rule,
            IdeationTargetType::Topic => Self::Topic,
            IdeationTargetType::Question => Self::Question,
            IdeationTargetType::Domain => Self::Domain,
            IdeationTargetType::Boundary => Self::Boundary,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum CanonicalArtifactType {
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "requirement")]
    Requirement,
    #[serde(rename = "resolution")]
    Resolution,
    #[serde(rename = "rule")]
    Rule,
}

impl CanonicalArtifactType {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        parse_enum_word(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum IdentityType {
    #[serde(rename = "human")]
    Human,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "service")]
    Service,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum IdeationEvidenceType {
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "artifact")]
    Artifact,
    #[serde(rename = "thread_message")]
    ThreadMessage,
    #[serde(rename = "domain_knowledge")]
    DomainKnowledge,
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "exploratory")]
    Exploratory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ContributionStance {
    #[serde(rename = "support")]
    Support,
    #[serde(rename = "oppose")]
    Oppose,
    #[serde(rename = "mixed")]
    Mixed,
    #[serde(rename = "needs_more_evidence")]
    NeedsMoreEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ArtifactChangeType {
    #[serde(rename = "create")]
    Create,
    #[serde(rename = "update")]
    Update,
    #[serde(rename = "remove")]
    Remove,
    #[serde(rename = "none")]
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum SpeculationMarker {
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "exploratory")]
    Exploratory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum UncertaintyLevel {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum EvidenceQuality {
    #[serde(rename = "strong")]
    Strong,
    #[serde(rename = "mixed")]
    Mixed,
    #[serde(rename = "weak")]
    Weak,
    #[serde(rename = "unsupported")]
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ProposalType {
    #[serde(rename = "requirement_candidate")]
    RequirementCandidate,
    #[serde(rename = "resolution_candidate")]
    ResolutionCandidate,
    #[serde(rename = "rule_candidate")]
    RuleCandidate,
    #[serde(rename = "source_gap")]
    SourceGap,
    #[serde(rename = "question")]
    Question,
    #[serde(rename = "no_action")]
    NoAction,
    #[serde(rename = "record_revision")]
    RecordRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum PromotionState {
    #[serde(rename = "proposed")]
    Proposed,
    #[serde(rename = "asserted")]
    Asserted,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "deferred")]
    Deferred,
    #[serde(rename = "duplicate")]
    Duplicate,
    #[serde(rename = "superseded")]
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum DispositionDecision {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "deferred")]
    Deferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IdeationTarget {
    #[serde(alias = "artifactType")]
    pub artifact_type: IdeationTargetType,
    #[serde(alias = "artifactId")]
    pub artifact_id: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IdeationEvidenceReference {
    #[serde(alias = "referenceId")]
    pub reference_id: StableId,
    #[serde(alias = "evidenceType")]
    pub evidence_type: IdeationEvidenceType,
    pub summary: String,
    #[serde(default, alias = "filePath", skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}
