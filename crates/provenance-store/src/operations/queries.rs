//! The eight structured query operations.
//!
//! Every primitive is one named operation with typed parameters and a
//! bounded answer. Each resolves its repository, reads state, and writes
//! nothing.

use camino::Utf8PathBuf;
use provenance_core::protocol::{
    EvidenceQuery, EvidenceResult, GetQuery, GetResult, ImpactQuery, ImpactResult, NeighborsQuery,
    NeighborsResult, ResolveSymbolQuery, ResolveSymbolResult, SearchQuery, SearchResult,
    StaleQuery, StaleResult, TraceQuery, TraceResult,
};
use provenance_core::ScopeId;

use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;

mod bindings;
mod evidence;
mod impact;
mod records;
mod stale;
mod symbols;
mod walk;

fn open(repo: Option<Utf8PathBuf>) -> anyhow::Result<(Utf8PathBuf, StateStore)> {
    let repo = super::discover_repository(repo)?;
    let store = StateStore::new(ProvenanceLayout::new(repo.clone()));
    Ok((repo, store))
}

pub fn get(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: GetQuery,
) -> anyhow::Result<GetResult> {
    let (_, store) = open(repo)?;
    records::get(&store, scope, request)
}

pub fn search(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: SearchQuery,
) -> anyhow::Result<SearchResult> {
    let (_, store) = open(repo)?;
    records::search(&store, scope, request)
}

pub fn neighbors(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: NeighborsQuery,
) -> anyhow::Result<NeighborsResult> {
    let (_, store) = open(repo)?;
    walk::neighbors(&store, scope, request)
}

pub fn trace(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: TraceQuery,
) -> anyhow::Result<TraceResult> {
    let (_, store) = open(repo)?;
    walk::trace(&store, scope, request)
}

pub fn impact(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: ImpactQuery,
) -> anyhow::Result<ImpactResult> {
    let (repo, store) = open(repo)?;
    impact::impact(&repo, &store, scope, request)
}

pub fn evidence(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: EvidenceQuery,
) -> anyhow::Result<EvidenceResult> {
    let (repo, store) = open(repo)?;
    evidence::evidence(&repo, &store, scope, request)
}

pub fn stale(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: StaleQuery,
) -> anyhow::Result<StaleResult> {
    let (repo, _) = open(repo)?;
    stale::stale(&repo, scope, request)
}

pub fn resolve_symbol(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: ResolveSymbolQuery,
) -> anyhow::Result<ResolveSymbolResult> {
    let (repo, store) = open(repo)?;
    symbols::resolve(&repo, &store, scope, request)
}
