//! The eight structured query operations.
//!
//! Every primitive is one named operation with typed parameters and a
//! bounded answer. Each resolves its repository, reads through the reader
//! entry under the supplied policy. Every answer carries a stamp.

use provenance_core::protocol::{
    EvidenceQuery, EvidenceResult, GetQuery, GetResult, ImpactQuery, ImpactResult, NeighborsQuery,
    NeighborsResult, ResolveRecordQuery, ResolveRecordResult, ResolveSymbolQuery,
    ResolveSymbolResult, SearchQuery, SearchResult, StaleQuery, StaleResult, Stamped, TraceQuery,
    TraceResult,
};

use super::read_policy::ReadPolicy;
use super::reader::{self, ReadContext, ReadFuture};
use crate::repository::Repository;

mod document;
pub(crate) use document::read_document_answer;
pub use document::{containing_review_documents, read_document};
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
    repository: &Repository,
    policy: ReadPolicy,
    run: impl for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
) -> anyhow::Result<Stamped<R>> {
    reader::answer(repository, policy, run).await
}

pub async fn get(
    repository: &Repository,
    policy: ReadPolicy,
    request: GetQuery,
) -> anyhow::Result<Stamped<GetResult>> {
    get_answer(repository, policy, request)
        .await
        .and_then(|answer| page::checked("get", answer))
}

pub(crate) async fn get_answer(
    repository: &Repository,
    policy: ReadPolicy,
    request: GetQuery,
) -> anyhow::Result<Stamped<GetResult>> {
    served(repository, policy, move |ctx| {
        Box::pin(async move { records::get(ctx, request).await })
    })
    .await
}

pub async fn resolve_record(
    repository: &Repository,
    policy: ReadPolicy,
    request: ResolveRecordQuery,
) -> anyhow::Result<Stamped<ResolveRecordResult>> {
    resolve_record_answer(repository, policy, request)
        .await
        .and_then(|answer| page::checked("resolve-record", answer))
}

pub(crate) async fn resolve_record_answer(
    repository: &Repository,
    policy: ReadPolicy,
    request: ResolveRecordQuery,
) -> anyhow::Result<Stamped<ResolveRecordResult>> {
    served(repository, policy, move |ctx| {
        Box::pin(async move { records::resolve_record(ctx, request).await })
    })
    .await
}

pub async fn search(
    repository: &Repository,
    policy: ReadPolicy,
    request: SearchQuery,
) -> anyhow::Result<Stamped<SearchResult>> {
    search_answer(repository, policy, request)
        .await
        .and_then(|answer| page::checked("search", answer))
}

pub(crate) async fn search_answer(
    repository: &Repository,
    policy: ReadPolicy,
    request: SearchQuery,
) -> anyhow::Result<Stamped<SearchResult>> {
    served(repository, policy, move |ctx| {
        Box::pin(async move { records::search(ctx, request).await })
    })
    .await
}

pub async fn neighbors(
    repository: &Repository,
    policy: ReadPolicy,
    request: NeighborsQuery,
) -> anyhow::Result<Stamped<NeighborsResult>> {
    neighbors_answer(repository, policy, request)
        .await
        .and_then(|answer| page::checked("neighbors", answer))
}

pub(crate) async fn neighbors_answer(
    repository: &Repository,
    policy: ReadPolicy,
    request: NeighborsQuery,
) -> anyhow::Result<Stamped<NeighborsResult>> {
    served(repository, policy, move |ctx| {
        Box::pin(async move { walk::neighbors(ctx, request).await })
    })
    .await
}

pub async fn trace(
    repository: &Repository,
    policy: ReadPolicy,
    request: TraceQuery,
) -> anyhow::Result<Stamped<TraceResult>> {
    trace_answer(repository, policy, request)
        .await
        .and_then(|answer| page::checked("trace", answer))
}

pub(crate) async fn trace_answer(
    repository: &Repository,
    policy: ReadPolicy,
    request: TraceQuery,
) -> anyhow::Result<Stamped<TraceResult>> {
    served(repository, policy, move |ctx| {
        Box::pin(async move { walk::trace(ctx, request).await })
    })
    .await
}

pub async fn impact(
    repository: &Repository,
    policy: ReadPolicy,
    request: ImpactQuery,
) -> anyhow::Result<Stamped<ImpactResult>> {
    impact_answer(repository, policy, request)
        .await
        .and_then(|answer| page::checked("impact", answer))
}

pub(crate) async fn impact_answer(
    repository: &Repository,
    policy: ReadPolicy,
    request: ImpactQuery,
) -> anyhow::Result<Stamped<ImpactResult>> {
    served(repository, policy, move |ctx| {
        Box::pin(async move { impact::impact(ctx, request).await })
    })
    .await
}

pub async fn evidence(
    repository: &Repository,
    policy: ReadPolicy,
    request: EvidenceQuery,
) -> anyhow::Result<Stamped<EvidenceResult>> {
    evidence_answer(repository, policy, request)
        .await
        .and_then(|answer| page::checked("evidence", answer))
}

pub(crate) async fn evidence_answer(
    repository: &Repository,
    policy: ReadPolicy,
    request: EvidenceQuery,
) -> anyhow::Result<Stamped<EvidenceResult>> {
    served(repository, policy, move |ctx| {
        Box::pin(async move { evidence::evidence(ctx, request).await })
    })
    .await
}

pub async fn stale(
    repository: &Repository,
    policy: ReadPolicy,
    request: StaleQuery,
) -> anyhow::Result<Stamped<StaleResult>> {
    stale_answer(repository, policy, request)
        .await
        .and_then(|answer| page::checked("stale", answer))
}

pub(crate) async fn stale_answer(
    repository: &Repository,
    policy: ReadPolicy,
    request: StaleQuery,
) -> anyhow::Result<Stamped<StaleResult>> {
    let inner = repository.scope().clone();
    served(repository, policy, move |ctx| {
        Box::pin(async move { stale::stale(ctx, &inner, request) })
    })
    .await
}

pub async fn resolve_symbol(
    repository: &Repository,
    policy: ReadPolicy,
    request: ResolveSymbolQuery,
) -> anyhow::Result<Stamped<ResolveSymbolResult>> {
    resolve_symbol_answer(repository, policy, request)
        .await
        .and_then(|answer| page::checked("resolve-symbol", answer))
}

pub(crate) async fn resolve_symbol_answer(
    repository: &Repository,
    policy: ReadPolicy,
    request: ResolveSymbolQuery,
) -> anyhow::Result<Stamped<ResolveSymbolResult>> {
    served(repository, policy, move |ctx| {
        Box::pin(async move { symbols::resolve(ctx, request).await })
    })
    .await
}
