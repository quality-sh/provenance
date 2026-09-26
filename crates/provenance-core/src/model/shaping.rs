use provenance_macros::{ProjectionRow, Relations};
use serde::{Deserialize, Serialize};

use super::artifacts::SourceReference;
use super::ids::{SchemaVersion, ScopeId, StableId};

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactLinkTargetType {
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "requirement")]
    Requirement,
    #[serde(rename = "resolution")]
    Resolution,
    #[serde(rename = "rule")]
    Rule,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopicStatus {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "explored")]
    Explored,
    #[serde(rename = "closed")]
    Closed,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestionStatus {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "blocked_on_human", alias = "blocked-on-human")]
    BlockedOnHuman,
    #[serde(rename = "answered")]
    Answered,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionMethod {
    #[serde(rename = "grill")]
    Grill,
    #[serde(rename = "prototype")]
    Prototype,
    #[serde(rename = "research")]
    Research,
    #[serde(rename = "verify")]
    Verify,
    #[serde(rename = "task")]
    Task,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLink {
    #[serde(alias = "targetType")]
    pub target_type: ArtifactLinkTargetType,
    #[serde(alias = "targetId")]
    pub target_id: StableId,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Relations, ProjectionRow)]
#[table("boundaries")]
pub struct Boundary {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    #[relation(target = Requirement, flow = none)]
    #[serde(alias = "requirementId")]
    pub requirement_id: StableId,
    pub statement: String,
    #[relation(target = Source, flow = none, name = "cites", via = source_id)]
    #[serde(default, alias = "sourceRef", skip_serializing_if = "Option::is_none")]
    #[column(json)]
    pub source_ref: Option<SourceReference>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Relations, ProjectionRow)]
#[table("topics")]
pub struct Topic {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    #[relation(target = Requirement, flow = none)]
    #[serde(alias = "requirementId")]
    pub requirement_id: StableId,
    pub title: String,
    pub status: TopicStatus,
    #[serde(default, alias = "claimedBy", skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    #[serde(default, alias = "claimedAt", skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<i64>,
    #[serde(default)]
    pub links: Vec<ArtifactLink>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Relations, ProjectionRow)]
#[table("questions")]
pub struct Question {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    #[relation(target = Topic, flow = none)]
    #[serde(alias = "topicId")]
    pub topic_id: StableId,
    #[relation(target = Requirement, flow = none)]
    #[serde(alias = "requirementId")]
    pub requirement_id: StableId,
    pub question: String,
    /// The verb that resolves this question, chosen when the question is minted.
    #[serde(alias = "resolutionMethod")]
    pub resolution_method: ResolutionMethod,
    pub status: QuestionStatus,
    #[serde(default, alias = "claimedBy", skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    #[serde(default, alias = "claimedAt", skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(default)]
    pub links: Vec<ArtifactLink>,
    #[relation(target = Resolution, flow = none)]
    #[serde(
        default,
        alias = "resolutionId",
        skip_serializing_if = "Option::is_none"
    )]
    pub resolution_id: Option<StableId>,
    #[relation(target = Requirement, flow = none)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contradicts: Option<StableId>,
}
