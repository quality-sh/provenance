//! The eight structured query operations, served from the projection.
//!
//! Every primitive is one named operation with typed parameters and a
//! bounded answer. Each resolves its repository, takes the reader policy,
//! and answers from the stamped projection inside one held publication
//! guard; the freshness stamp travels with every answer. The pre-re-back
//! executors survive verbatim beside the tests for the differential
//! harness.

use camino::Utf8PathBuf;
use provenance_core::protocol::{
    EvidenceQuery, EvidenceResult, GetQuery, GetResult, ImpactQuery, ImpactResult, NeighborsQuery,
    NeighborsResult, ResolveSymbolQuery, ResolveSymbolResult, SearchQuery, SearchResult,
    StaleQuery, StaleResult, TraceQuery, TraceResult,
};
use provenance_core::ScopeId;

mod served_evidence;
mod served_graph;
mod served_live;
mod stale;
pub(crate) mod trace_token;

/// Fetch one record by canonical id.
pub async fn get(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: GetQuery,
) -> anyhow::Result<GetResult> {
    served_graph::get(repo, scope, request).await
}

/// Find records whose text contains a phrase.
pub async fn search(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: SearchQuery,
) -> anyhow::Result<SearchResult> {
    served_graph::search(repo, scope, request).await
}

/// Read the records one hop from a record.
pub async fn neighbors(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: NeighborsQuery,
) -> anyhow::Result<NeighborsResult> {
    served_graph::neighbors(repo, scope, request).await
}

/// Walk outward from a record for a bounded number of hops.
pub async fn trace(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: TraceQuery,
) -> anyhow::Result<TraceResult> {
    served_graph::trace(repo, scope, request).await
}

/// Read the Rules a record reaches, with the code standing behind them.
pub async fn impact(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: ImpactQuery,
) -> anyhow::Result<ImpactResult> {
    served_live::impact(repo, scope, request).await
}

/// Read everything standing behind one Rule.
pub async fn evidence(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: EvidenceQuery,
) -> anyhow::Result<EvidenceResult> {
    served_evidence::evidence(repo, scope, request).await
}

/// Read which evidence sites a commit range disturbed.
pub fn stale(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: StaleQuery,
) -> anyhow::Result<StaleResult> {
    served_live::stale(repo, scope, request)
}

/// Read the Rules bound to one code site.
pub async fn resolve_symbol(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: ResolveSymbolQuery,
) -> anyhow::Result<ResolveSymbolResult> {
    served_live::resolve_symbol(repo, scope, request).await
}

#[cfg(test)]
mod tests;
