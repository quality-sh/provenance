use camino::Utf8Path;
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, ImpactQuery, ImpactResult, TRACE_MAX_DEPTH,
};
use provenance_core::{NodeType, ScopeId, StableId};
use provenance_store::state_store::StateStore;
use std::collections::BTreeSet;

use super::super::sites;
use super::{bindings::Bindings, records, walk};

/// Names every Rule a record reaches, with the code standing behind it.
///
/// A Requirement reaches its Rules directly; a Source reaches them through
/// the Requirements it grounds. The walk is bounded by the same depth cap
/// `trace` uses, so no request can pull the whole graph back.
pub(super) fn impact(
    repo: &Utf8Path,
    store: &StateStore,
    scope: &ScopeId,
    request: ImpactQuery,
) -> anyhow::Result<ImpactResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let id = StableId::new(request.id.clone())?;
    let nodes = records::load(store, scope, request.include_retired)?;
    let edges = walk::scoped_edges(store, scope)?;
    let mut seen = BTreeSet::from([id.as_str().to_string()]);
    let mut rules = BTreeSet::new();
    if records::find(&nodes, Some(NodeType::Rule), &id).is_some() {
        rules.insert(id.as_str().to_string());
    }
    let mut frontier = vec![id];
    for _ in 0..TRACE_MAX_DEPTH {
        let mut next = Vec::new();
        for origin in &frontier {
            for edge in &edges {
                let reached = edge.from_id == *origin;
                if !reached {
                    continue;
                }
                if !seen.insert(edge.to_id.as_str().to_string()) {
                    continue;
                }
                if records::find(&nodes, Some(edge.to_type), &edge.to_id).is_none() {
                    continue;
                }
                if edge.to_type == NodeType::Rule {
                    rules.insert(edge.to_id.as_str().to_string());
                }
                next.push(edge.to_id.clone());
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    let wanted = rules
        .into_iter()
        .take(request.limit + 1)
        .collect::<Vec<_>>();
    let (wanted, has_more) = take_page(wanted, request.limit);
    let bindings = Bindings::load(store, scope, request.include_retired)?;
    let scans = provenance_scanner::scan_path(repo)?;
    let evidence = sites::Evidence {
        scans: &scans,
        verifications: &bindings.verifications,
        implementations: &bindings.implementations,
    };
    let affected_rules = wanted
        .into_iter()
        .map(|rule| Ok(evidence.affected_rule(repo, StableId::new(rule)?)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(ImpactResult {
        id: request.id,
        limit: request.limit,
        has_more,
        affected_rules,
    })
}
