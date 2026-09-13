use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

use crate::model::{
    Boundary, Domain, NodeType, Question, Requirement, Resolution, Rule, Source, StableId, Topic,
};

use super::Direction;

/// One canonical record as a query hands it back.
///
/// Each variant carries the record the store already writes, so a primitive
/// never invents a second vocabulary for a Requirement or a Rule. The
/// `node_type` tag is the same word a relation row uses for its endpoints.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "node_type", rename_all = "snake_case")]
pub enum GraphNode {
    Source(Box<Source>),
    Requirement(Box<Requirement>),
    Resolution(Box<Resolution>),
    Rule(Box<Rule>),
    Topic(Box<Topic>),
    Question(Box<Question>),
    Domain(Box<Domain>),
    Boundary(Box<Boundary>),
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
            Self::Domain(_) => NodeType::Domain,
            Self::Boundary(_) => NodeType::Boundary,
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
            Self::Domain(record) => &record.id,
            Self::Boundary(record) => &record.id,
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
            Self::Domain(record) => {
                text.push(record.name.as_str());
                text.extend(record.description.as_deref());
            }
            Self::Boundary(record) => text.push(record.statement.as_str()),
        }
        text
    }
}

/// One record reached in a single hop, with the relation that reached it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Neighbor {
    pub relation: String,
    pub direction: Direction,
    pub node: GraphNode,
}

/// One record reached by a walk, with how many hops it took.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TracedNode {
    pub depth: usize,
    pub node: GraphNode,
}

/// Where a Rule is implemented.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Deserialize, Serialize)]
pub struct ImplementationSite {
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub file: Utf8PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// Where a Rule is verified, and how.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Deserialize, Serialize)]
pub struct VerificationSite {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_by: Option<String>,
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
    pub file: Utf8PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// One Rule a change reaches, with the code that stands behind it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
