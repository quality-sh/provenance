use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::coverage::{EvidenceDiffSite, EvidenceDiffSummary};
use crate::model::{
    ImplementationBinding, RequirementReview, VerificationBinding, VerificationRun,
};

use super::{AffectedRule, GraphNode, Neighbor, Stamp, Stamped, TracedNode, SDK_PROTOCOL_VERSION};

/// The envelope every query primitive answers in.
///
/// The protocol version travels with the answer, so a caller holding a
/// recorded response can tell which contract produced it, `operation`
/// names which primitive it came from, and `stamp` says what the answer
/// reflects.
#[derive(Debug, Clone, Serialize)]
// The envelope stays encode-only: `operation` is a static name.
pub struct QueryResponse<Result> {
    pub protocol_version: u32,
    pub operation: &'static str,
    pub stamp: Stamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness_error: Option<String>,
    #[serde(flatten)]
    pub result: Result,
}

impl<Result> QueryResponse<Result> {
    pub fn new(operation: &'static str, answer: Stamped<Result>) -> Self {
        Self {
            protocol_version: SDK_PROTOCOL_VERSION,
            operation,
            stamp: answer.stamp,
            freshness_error: answer.freshness_error,
            result: answer.result,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GetResult {
    pub found: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<GraphNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchResult {
    pub limit: usize,
    pub has_more: bool,
    pub nodes: Vec<GraphNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NeighborsResult {
    pub id: String,
    pub limit: usize,
    pub has_more: bool,
    pub neighbors: Vec<Neighbor>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TraceResult {
    pub id: String,
    pub max_depth: usize,
    pub limit: usize,
    pub has_more: bool,
    pub nodes: Vec<TracedNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImpactResult {
    pub id: String,
    pub limit: usize,
    pub has_more: bool,
    pub affected_rules: Vec<AffectedRule>,
    /// The working-tree scan stopped at the configured file count, so the
    /// scanned sites are a lower bound.
    #[serde(default)]
    pub scan_cut: bool,
}

/// What a commit range did to the code carrying a Rule's evidence.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaleEvidence {
    pub base: String,
    pub head: String,
    pub sites: Vec<EvidenceDiffSite>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvidenceResult {
    pub rule_id: String,
    pub limit: usize,
    pub has_more: bool,
    pub implementation_bindings: Vec<ImplementationBinding>,
    pub verification_bindings: Vec<VerificationBinding>,
    pub verification_runs: Vec<VerificationRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_verification_run: Option<VerificationRun>,
    pub review_required: bool,
    pub reviews: Vec<RequirementReview>,
    pub stale: Option<StaleEvidence>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaleResult {
    pub base: String,
    pub head: String,
    pub files_changed: usize,
    pub summary: EvidenceDiffSummary,
    pub limit: usize,
    pub has_more: bool,
    pub sites: Vec<EvidenceDiffSite>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResolveSymbolResult {
    pub file: Utf8PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub limit: usize,
    pub has_more: bool,
    pub rules: Vec<GraphNode>,
}
