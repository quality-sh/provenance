use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::model::NodeType;

use super::{QUERY_DEFAULT_LIMIT, TRACE_DEFAULT_MAX_DEPTH};

const fn default_limit() -> usize {
    QUERY_DEFAULT_LIMIT
}

const fn default_max_depth() -> usize {
    TRACE_DEFAULT_MAX_DEPTH
}

/// Which way a query follows a relation.
///
/// `out` reads the relations the named record holds in its own fields, `in`
/// reads the relations other records hold toward it, and `both` reads every
/// relation from either end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Out,
    In,
    #[default]
    Both,
}

impl Direction {
    pub const fn reads_out(self) -> bool {
        matches!(self, Self::Out | Self::Both)
    }

    pub const fn reads_in(self) -> bool {
        matches!(self, Self::In | Self::Both)
    }
}

/// Fetch one record by canonical ID.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetQuery {
    #[serde(default)]
    pub protocol_version: Option<u32>,
    pub node_type: NodeType,
    pub id: String,
    #[serde(default)]
    pub include_retired: bool,
}

/// Find records whose text contains a phrase.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchQuery {
    #[serde(default)]
    pub protocol_version: Option<u32>,
    pub text: String,
    #[serde(default)]
    pub node_types: Vec<NodeType>,
    #[serde(default)]
    pub include_retired: bool,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Read the records one hop from a record.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NeighborsQuery {
    #[serde(default)]
    pub protocol_version: Option<u32>,
    pub id: String,
    #[serde(default)]
    pub node_type: Option<NodeType>,
    #[serde(default)]
    pub direction: Direction,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub include_retired: bool,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Walk outward from a record for a bounded number of hops.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceQuery {
    #[serde(default)]
    pub protocol_version: Option<u32>,
    pub id: String,
    #[serde(default)]
    pub node_type: Option<NodeType>,
    #[serde(default)]
    pub direction: Direction,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
    #[serde(default)]
    pub include_retired: bool,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Read the Rules a record reaches, with the code standing behind them.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImpactQuery {
    #[serde(default)]
    pub protocol_version: Option<u32>,
    pub id: String,
    #[serde(default)]
    pub node_type: Option<NodeType>,
    #[serde(default)]
    pub include_retired: bool,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Read everything standing behind one Rule.
///
/// `base` is what makes the stale report computable: stale means the code
/// carrying the evidence changed, and that is read from a diff. Without a
/// base the response says so rather than guessing.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceQuery {
    #[serde(default)]
    pub protocol_version: Option<u32>,
    pub rule: String,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub include_retired: bool,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Read which evidence sites a commit range disturbed.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaleQuery {
    #[serde(default)]
    pub protocol_version: Option<u32>,
    pub base: String,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub include_retired: bool,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// Read the Rules bound to one code site.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveSymbolQuery {
    #[serde(default)]
    pub protocol_version: Option<u32>,
    pub file: Utf8PathBuf,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub line: Option<usize>,
    #[serde(default)]
    pub include_retired: bool,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
