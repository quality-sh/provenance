use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

use crate::model::{
    EdgeType, NodeType, Question, Requirement, Resolution, Rule, Source, StableId, Topic,
};

use super::Direction;

/// One canonical record as a query hands it back.
///
/// Each variant carries the record the store already writes, so a primitive
/// never invents a second vocabulary for a Requirement or a Rule. The
/// `node_type` tag is the same word `Edge` uses for its endpoints.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "node_type", rename_all = "snake_case")]
pub enum GraphNode {
    Source(Box<Source>),
    Requirement(Box<Requirement>),
    Resolution(Box<Resolution>),
    Rule(Box<Rule>),
    Topic(Box<Topic>),
    Question(Box<Question>),
}

impl GraphNode {
    pub const fn node_type(&self) -> NodeType {
        match self {
            Self::Source(_) => NodeType::Source,
            Self::Requirement(_) => NodeType::Requirement,
            Self::Resolution(_) => NodeType::Resolution,
            Self::Rule(_) => NodeType::Rule,
            Self::Topic(_) => NodeType::Topic,
            Self::Question(_) => NodeType::Question,
        }
    }

    pub fn id(&self) -> &StableId {
        match self {
            Self::Source(record) => &record.id,
            Self::Requirement(record) => &record.id,
            Self::Resolution(record) => &record.id,
            Self::Rule(record) => &record.id,
            Self::Topic(record) => &record.id,
            Self::Question(record) => &record.id,
        }
    }

    /// Whether an active view should leave this record out.
    ///
    /// Only the record kinds that retire in place carry the flag; the shaping
    /// kinds have their own status words and are never retired.
    pub const fn retired(&self) -> bool {
        match self {
            Self::Source(record) => record.retired,
            Self::Requirement(record) => record.retired,
            Self::Rule(record) => record.retired,
            Self::Resolution(_) | Self::Topic(_) | Self::Question(_) => false,
        }
    }

    /// The words a text search reads on this record.
    pub fn searchable_text(&self) -> Vec<&str> {
        let mut text = vec![self.id().as_str()];
        match self {
            Self::Source(record) => {
                text.push(record.name.as_str());
                text.extend(record.reference.as_deref());
            }
            Self::Requirement(record) => {
                text.push(record.statement.as_str());
                text.extend(record.description.as_deref());
            }
            Self::Resolution(record) => {
                text.push(record.title.as_str());
                text.push(record.position.as_str());
            }
            Self::Rule(record) => {
                text.push(record.statement.as_str());
                text.extend(record.name.as_deref());
                text.extend(record.description.as_deref());
            }
            Self::Topic(record) => text.push(record.title.as_str()),
            Self::Question(record) => text.push(record.question.as_str()),
        }
        text
    }
}

/// One record reached in a single hop, with the edge that reached it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Neighbor {
    pub edge_type: EdgeType,
    pub direction: Direction,
    pub node: GraphNode,
}

/// One record reached by a walk, with how many hops it took.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TracedNode {
    pub depth: usize,
    pub node: GraphNode,
}

/// Where a Rule is implemented.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Deserialize, Serialize)]
pub struct ImplementationSite {
    pub file: Utf8PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// Where a Rule is verified, and how.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Deserialize, Serialize)]
pub struct VerificationSite {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_by: Option<String>,
    pub file: Utf8PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// One Rule a change reaches, with the code that stands behind it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AffectedRule {
    pub id: StableId,
    pub implementations: Vec<ImplementationSite>,
    pub verifications: Vec<VerificationSite>,
}

impl ImplementationSite {
    /// Names where a reader should look, as precisely as the record allows.
    pub fn location(&self) -> String {
        match (self.line, self.symbol.as_deref()) {
            (Some(line), _) => format!("{}:{line}", self.file),
            (None, Some(symbol)) => format!("{} ({symbol})", self.file),
            (None, None) => self.file.to_string(),
        }
    }
}

impl VerificationSite {
    /// Names the test site and how it checks the Rule.
    pub fn location(&self) -> String {
        let mut location = self.line.map_or_else(
            || self.file.to_string(),
            |line| format!("{}:{line}", self.file),
        );
        if let Some(symbol) = &self.symbol {
            let _ = write!(location, " ({symbol})");
        }
        self.key.as_ref().map_or_else(
            || format!("{location} [{}]", self.method),
            |key| format!("{location} [{key}, {}]", self.method),
        )
    }
}
