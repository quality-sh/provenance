//! The eight structured query operations.
//!
//! Every primitive is one named operation with typed parameters and a
//! bounded answer. Each resolves its repository, reads through the reader
//! entry under the default policy, and writes nothing. Every answer carries
//! a stamp; before an operation moves onto the projection its stamp attests
//! nothing and names `canonical` among its live words.

use camino::Utf8PathBuf;
use provenance_core::protocol::{
    EvidenceQuery, EvidenceResult, GetQuery, GetResult, ImpactQuery, ImpactResult, NeighborsQuery,
    NeighborsResult, ResolveSymbolQuery, ResolveSymbolResult, SearchQuery, SearchResult,
    StaleQuery, StaleResult, Stamped, TraceQuery, TraceResult,
};
use provenance_core::ScopeId;

use super::read_policy::ReadPolicy;
use super::reader::{self, ReadContext, ReadFuture};

mod bindings;
mod evidence;
mod impact;
mod nodes;
mod records;
mod stale;
mod symbols;
mod walk;

#[cfg(test)]
mod tests;

async fn served<R: Send>(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    run: impl for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
) -> anyhow::Result<Stamped<R>> {
    let repo = super::discover_repository(repo)?;
    reader::answer(&repo, scope, ReadPolicy::default(), run).await
}

pub async fn get(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: GetQuery,
) -> anyhow::Result<Stamped<GetResult>> {
    served(repo, scope, move |ctx| {
        Box::pin(async move { records::get(ctx, request).await })
    })
    .await
}

pub async fn search(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: SearchQuery,
) -> anyhow::Result<Stamped<SearchResult>> {
    served(repo, scope, move |ctx| {
        Box::pin(async move { records::search(ctx, request).await })
    })
    .await
}

pub async fn neighbors(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: NeighborsQuery,
) -> anyhow::Result<Stamped<NeighborsResult>> {
    served(repo, scope, move |ctx| {
        Box::pin(async move { walk::neighbors(ctx, request).await })
    })
    .await
}

pub async fn trace(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: TraceQuery,
) -> anyhow::Result<Stamped<TraceResult>> {
    served(repo, scope, move |ctx| {
        Box::pin(async move { walk::trace(ctx, request).await })
    })
    .await
}

pub async fn impact(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: ImpactQuery,
) -> anyhow::Result<Stamped<ImpactResult>> {
    served(repo, scope, move |ctx| {
        Box::pin(async move { impact::impact(ctx, request).await })
    })
    .await
}

pub async fn evidence(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: EvidenceQuery,
) -> anyhow::Result<Stamped<EvidenceResult>> {
    let inner = scope.clone();
    served(repo, scope, move |ctx| {
        Box::pin(async move { evidence::evidence(ctx, &inner, request) })
    })
    .await
}

pub async fn stale(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: StaleQuery,
) -> anyhow::Result<Stamped<StaleResult>> {
    let inner = scope.clone();
    served(repo, scope, move |ctx| {
        Box::pin(async move { stale::stale(ctx, &inner, request) })
    })
    .await
}

pub async fn resolve_symbol(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: ResolveSymbolQuery,
) -> anyhow::Result<Stamped<ResolveSymbolResult>> {
    let inner = scope.clone();
    served(repo, scope, move |ctx| {
        Box::pin(async move { symbols::resolve(ctx, &inner, request) })
    })
    .await
}
