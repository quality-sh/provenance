mod node;
mod query;
mod response;
mod stamp;
#[cfg(test)]
mod stamp_tests;

pub use stamp::{
    decode_cursor, encode_cursor, resolve_budget, AttestedDomain, FreshnessPolicy, FreshnessStamp,
    LiveConstituent, SCAN_BUDGET_CAP, SCAN_BUDGET_DEFAULT, VISIT_BUDGET_CAP, VISIT_BUDGET_DEFAULT,
};
mod typed_spec;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

pub use node::{
    AffectedRule, GraphNode, ImplementationSite, Neighbor, TracedNode, VerificationSite,
};
pub use query::{
    Direction, EvidenceQuery, GetQuery, ImpactQuery, NeighborsQuery, ResolveSymbolQuery,
    SearchQuery, StaleQuery, TraceQuery,
};
pub use response::{
    CollectionPage, EvidenceResult, GetResult, ImpactResult, NeighborsResult, QueryResponse,
    ResolveSymbolResult, SearchResult, StaleEvidence, StaleResult, TraceResult,
};
pub use typed_spec::{
    CheckStatementRequest, TypedAdoptionTarget, TypedDeclarationKind, TypedImplementationInput,
    TypedRequirementInput, TypedRuleInput, TypedSourceInput, TypedSpecInput,
};

pub const SDK_PROTOCOL_VERSION: u32 = 5;

/// How many records a query returns when the caller names no limit.
pub const QUERY_DEFAULT_LIMIT: usize = 50;

/// The largest page any query primitive will hand back.
pub const QUERY_MAX_LIMIT: usize = 200;

/// How many hops `trace` walks when the caller names no depth.
pub const TRACE_DEFAULT_MAX_DEPTH: usize = 3;

/// The deepest walk `trace` will make.
pub const TRACE_MAX_DEPTH: usize = 10;

/// Language-neutral contract advertised by the Rust engine before SDK work.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct EngineInfo {
    pub engine_version: String,
    pub protocol_version: u32,
    pub state_schema_version: u32,
    pub repository: Utf8PathBuf,
}

/// Refuses a request written against another protocol version.
///
/// A caller may leave `protocol_version` out and take whatever this engine
/// speaks, which is what the SDK handshake already checks. Naming a version
/// is a claim about the shapes being sent, so a mismatch is refused here
/// rather than parsed into a shape the caller did not mean.
pub fn ensure_protocol_version(requested: Option<u32>) -> anyhow::Result<()> {
    let Some(requested) = requested else {
        return Ok(());
    };
    anyhow::ensure!(
        requested == SDK_PROTOCOL_VERSION,
        "request names protocol version {requested}; this engine speaks {SDK_PROTOCOL_VERSION}"
    );
    Ok(())
}

/// Refuses a page nobody bounded.
pub fn ensure_limit(limit: usize) -> anyhow::Result<()> {
    anyhow::ensure!(
        (1..=QUERY_MAX_LIMIT).contains(&limit),
        "limit must be between 1 and {QUERY_MAX_LIMIT}"
    );
    Ok(())
}

/// Refuses a walk deeper than the engine will bound.
pub fn ensure_max_depth(depth: usize) -> anyhow::Result<()> {
    anyhow::ensure!(
        (1..=TRACE_MAX_DEPTH).contains(&depth),
        "max_depth must be between 1 and {TRACE_MAX_DEPTH}"
    );
    Ok(())
}

/// Cuts one over-long page down to its limit and says whether more remain.
pub fn take_page<T>(mut items: Vec<T>, limit: usize) -> (Vec<T>, bool) {
    let has_more = items.len() > limit;
    items.truncate(limit);
    (items, has_more)
}
