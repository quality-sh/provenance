use serde::{Deserialize, Serialize};

use super::ids::{SchemaVersion, ScopeId, StableId};
use super::parsing::normalize_enum_value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
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

impl NodeType {
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
                "parent type must be source, requirement, resolution, rule, topic, question, \
                 domain, or boundary"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeType {
    #[serde(rename = "references")]
    References,
    #[serde(rename = "refines_into")]
    RefinesInto,
    #[serde(rename = "depends_on")]
    DependsOn,
    #[serde(rename = "contradicts")]
    Contradicts,
    #[serde(rename = "supersedes")]
    Supersedes,
    #[serde(rename = "needs")]
    Needs,
    #[serde(rename = "resolves")]
    Resolves,
    #[serde(rename = "spawns")]
    Spawns,
    #[serde(rename = "produces")]
    Produces,
}

impl EdgeType {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "references" => Ok(Self::References),
            "refines_into" => Ok(Self::RefinesInto),
            "depends_on" => Ok(Self::DependsOn),
            "contradicts" => Ok(Self::Contradicts),
            "supersedes" => Ok(Self::Supersedes),
            "needs" => Ok(Self::Needs),
            "resolves" => Ok(Self::Resolves),
            "spawns" => Ok(Self::Spawns),
            "produces" => Ok(Self::Produces),
            _ => anyhow::bail!("edge type must be a supported provenance edge type"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub edge_type: EdgeType,
    pub from_type: NodeType,
    pub from_id: StableId,
    pub to_type: NodeType,
    pub to_id: StableId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Edge {
    pub fn stable_id(
        edge_type: EdgeType,
        from_type: NodeType,
        from_id: &StableId,
        to_type: NodeType,
        to_id: &StableId,
    ) -> anyhow::Result<StableId> {
        StableId::new(
            format!(
                "{:?}_{:?}_{}_to_{:?}_{}",
                edge_type,
                from_type,
                from_id.as_str(),
                to_type,
                to_id.as_str()
            )
            .to_ascii_lowercase(),
        )
    }
}
