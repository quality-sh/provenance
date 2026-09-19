//! The eight structured query operations.
//!
//! Every primitive is one named operation with typed parameters and a
//! bounded answer. Each resolves its repository, reads through the reader
//! entry under the supplied policy. Every answer carries a stamp.

use camino::Utf8PathBuf;
use provenance_core::protocol::{
    EvidenceQuery, EvidenceResult, GetQuery, GetResult, ImpactQuery, ImpactResult, NeighborsQuery,
    NeighborsResult, ResolveSymbolQuery, ResolveSymbolResult, SearchQuery, SearchResult,
    StaleQuery, StaleResult, Stamped, TraceQuery, TraceResult,
};
use provenance_core::ScopeId;

use super::read_policy::ReadPolicy;
use super::reader::{self, ReadContext, ReadFuture};

mod document;
pub use document::read_document;
pub(crate) use document::read_document_answer;
mod evidence;
mod impact;
mod nodes;
pub(crate) mod page;
mod records;
mod stale;
mod symbols;
mod walk;

#[cfg(test)]
mod tests;

async fn served<R: Send>(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    run: impl for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
) -> anyhow::Result<Stamped<R>> {
    let repo = super::discover_repository(repo)?;
    reader::answer(&repo, scope, policy, run).await
}

pub async fn get(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: GetQuery,
) -> anyhow::Result<Stamped<GetResult>> {
    get_answer(repo, scope, policy, request)
        .await
        .and_then(|answer| page::checked("get", answer))
}

pub(crate) async fn get_answer(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: GetQuery,
) -> anyhow::Result<Stamped<GetResult>> {
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { records::get(ctx, request).await })
    })
    .await
}

pub async fn search(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: SearchQuery,
) -> anyhow::Result<Stamped<SearchResult>> {
    search_answer(repo, scope, policy, request)
        .await
        .and_then(|answer| page::checked("search", answer))
}

pub(crate) async fn search_answer(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: SearchQuery,
) -> anyhow::Result<Stamped<SearchResult>> {
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { records::search(ctx, request).await })
    })
    .await
}

pub async fn neighbors(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: NeighborsQuery,
) -> anyhow::Result<Stamped<NeighborsResult>> {
    neighbors_answer(repo, scope, policy, request)
        .await
        .and_then(|answer| page::checked("neighbors", answer))
}

pub(crate) async fn neighbors_answer(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: NeighborsQuery,
) -> anyhow::Result<Stamped<NeighborsResult>> {
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { walk::neighbors(ctx, request).await })
    })
    .await
}

pub async fn trace(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: TraceQuery,
) -> anyhow::Result<Stamped<TraceResult>> {
    trace_answer(repo, scope, policy, request)
        .await
        .and_then(|answer| page::checked("trace", answer))
}

pub(crate) async fn trace_answer(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: TraceQuery,
) -> anyhow::Result<Stamped<TraceResult>> {
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { walk::trace(ctx, request).await })
    })
    .await
}

pub async fn impact(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: ImpactQuery,
) -> anyhow::Result<Stamped<ImpactResult>> {
    impact_answer(repo, scope, policy, request)
        .await
        .and_then(|answer| page::checked("impact", answer))
}

pub(crate) async fn impact_answer(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: ImpactQuery,
) -> anyhow::Result<Stamped<ImpactResult>> {
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { impact::impact(ctx, request).await })
    })
    .await
}

pub async fn evidence(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: EvidenceQuery,
) -> anyhow::Result<Stamped<EvidenceResult>> {
    evidence_answer(repo, scope, policy, request)
        .await
        .and_then(|answer| page::checked("evidence", answer))
}

pub(crate) async fn evidence_answer(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: EvidenceQuery,
) -> anyhow::Result<Stamped<EvidenceResult>> {
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { evidence::evidence(ctx, request).await })
    })
    .await
}

pub async fn stale(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: StaleQuery,
) -> anyhow::Result<Stamped<StaleResult>> {
    stale_answer(repo, scope, policy, request)
        .await
        .and_then(|answer| page::checked("stale", answer))
}

pub(crate) async fn stale_answer(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: StaleQuery,
) -> anyhow::Result<Stamped<StaleResult>> {
    let inner = scope.clone();
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { stale::stale(ctx, &inner, request) })
    })
    .await
}

pub async fn resolve_symbol(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: ResolveSymbolQuery,
) -> anyhow::Result<Stamped<ResolveSymbolResult>> {
    resolve_symbol_answer(repo, scope, policy, request)
        .await
        .and_then(|answer| page::checked("resolve-symbol", answer))
}

pub(crate) async fn resolve_symbol_answer(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: ResolveSymbolQuery,
) -> anyhow::Result<Stamped<ResolveSymbolResult>> {
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { symbols::resolve(ctx, request).await })
    })
    .await
}
