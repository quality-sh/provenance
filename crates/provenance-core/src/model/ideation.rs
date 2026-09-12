use serde::{Deserialize, Serialize};

pub(super) mod contributions;
pub(super) mod dispositions;
pub(super) mod legacy_audit;
pub(super) mod lifecycle;
pub(super) mod proposals;
pub(super) mod synthesis;

use super::graph::NodeType;
use super::ids::StableId;
use super::parsing::normalize_enum_value;

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

impl IdeationTargetType {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "source" => Ok(Self::Source),
            "requirement" => Ok(Self::Requirement),
            "resolution" => Ok(Self::Resolution),
            "rule" => Ok(Self::Rule),
            "topic" => Ok(Self::Topic),
            "question" => Ok(Self::Question),
            "domain" => Ok(Self::Domain),
            "boundary" => Ok(Self::Boundary),
            _ => anyhow::bail!(
                "target type must be source, requirement, resolution, rule, topic, question, \
                 domain, or boundary"
            ),
        }
    }
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
        match normalize_enum_value(value).as_str() {
            "source" => Ok(Self::Source),
            "requirement" => Ok(Self::Requirement),
            "resolution" => Ok(Self::Resolution),
            "rule" => Ok(Self::Rule),
            _ => anyhow::bail!(
                "canonical artifact type must be source, requirement, resolution, or rule"
            ),
        }
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

impl IdentityType {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            "service" => Ok(Self::Service),
            _ => anyhow::bail!("identity type must be human, agent, or service"),
        }
    }
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

impl ContributionStance {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "support" => Ok(Self::Support),
            "oppose" => Ok(Self::Oppose),
            "mixed" => Ok(Self::Mixed),
            "needs_more_evidence" => Ok(Self::NeedsMoreEvidence),
            _ => anyhow::bail!("stance must be support, oppose, mixed, or needs_more_evidence"),
        }
    }
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

impl UncertaintyLevel {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => anyhow::bail!("uncertainty level must be low, medium, or high"),
        }
    }
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

impl ProposalType {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "requirement_candidate" => Ok(Self::RequirementCandidate),
            "resolution_candidate" => Ok(Self::ResolutionCandidate),
            "rule_candidate" => Ok(Self::RuleCandidate),
            "source_gap" => Ok(Self::SourceGap),
            "question" => Ok(Self::Question),
            "no_action" => Ok(Self::NoAction),
            "record_revision" => Ok(Self::RecordRevision),
            _ => anyhow::bail!("proposal type must be requirement_candidate, resolution_candidate, rule_candidate, source_gap, question, no_action, or record_revision"),
        }
    }
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

impl PromotionState {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "proposed" => Ok(Self::Proposed),
            "asserted" => Ok(Self::Asserted),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "deferred" => Ok(Self::Deferred),
            "duplicate" => Ok(Self::Duplicate),
            "superseded" => Ok(Self::Superseded),
            _ => anyhow::bail!(
                "promotion state must be proposed, asserted, accepted, rejected, deferred, duplicate, or superseded"
            ),
        }
    }
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

impl DispositionDecision {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match normalize_enum_value(value).as_str() {
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "deferred" => Ok(Self::Deferred),
            _ => anyhow::bail!("disposition decision must be accepted, rejected, or deferred"),
        }
    }
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
