use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::coverage::{EvidenceDiffSite, EvidenceDiffSummary};
use crate::model::{
    ImplementationBinding, RequirementReview, VerificationBinding, VerificationRun,
};

use super::stamp::FreshnessStamp;
use super::{AffectedRule, GraphNode, Neighbor, TracedNode, SDK_PROTOCOL_VERSION};

/// Paging truth for one evidence collection.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionPage {
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The envelope every query primitive answers in.
///
/// The protocol version travels with the answer, so a caller holding a
/// recorded response can tell which contract produced it, and `operation`
/// names which primitive it came from.
#[derive(Debug, Clone, Serialize)]
// The envelope stays encode-only: `operation` is a static name.
pub struct QueryResponse<Result> {
    pub protocol_version: u32,
    pub operation: &'static str,
    #[serde(flatten)]
    pub result: Result,
}

impl<Result> QueryResponse<Result> {
    pub const fn new(operation: &'static str, result: Result) -> Self {
        Self {
            protocol_version: SDK_PROTOCOL_VERSION,
            operation,
            result,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GetResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<FreshnessStamp>,
    pub found: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<GraphNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<FreshnessStamp>,
    pub limit: usize,
    pub has_more: bool,
    pub nodes: Vec<GraphNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NeighborsResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<FreshnessStamp>,
    pub id: String,
    pub limit: usize,
    pub has_more: bool,
    pub neighbors: Vec<Neighbor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TraceResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<FreshnessStamp>,
    pub id: String,
    pub max_depth: usize,
    pub limit: usize,
    pub has_more: bool,
    pub nodes: Vec<TracedNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImpactResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<FreshnessStamp>,
    pub id: String,
    pub limit: usize,
    pub has_more: bool,
    pub affected_rules: Vec<AffectedRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<FreshnessStamp>,
    pub rule_id: String,
    pub limit: usize,
    pub has_more: bool,
    pub implementation_bindings_page: CollectionPage,
    pub verification_bindings_page: CollectionPage,
    pub verification_runs_page: CollectionPage,
    pub reviews_page: CollectionPage,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<FreshnessStamp>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stamp: Option<FreshnessStamp>,
    pub file: Utf8PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub limit: usize,
    pub has_more: bool,
    pub rules: Vec<GraphNode>,
}
