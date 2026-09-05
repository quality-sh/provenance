use crate::operations::reader::{Live, ReadContext};
use provenance_core::model::relations::flow_neighbors;
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, ImpactQuery, ImpactResult, TRACE_MAX_DEPTH,
};
use provenance_core::{NodeType, ScopeId, StableId};
use std::collections::BTreeSet;

use super::super::sites;
use super::{bindings::Bindings, walk};

/// Names every Rule a record reaches, with the code standing behind it.
///
/// A Requirement reaches its Rules directly; a Source reaches them through
/// the Requirements it grounds. The walk is bounded by the same depth cap
/// `trace` uses, so no request can pull the whole graph back.
pub(super) fn impact(
    ctx: &ReadContext,
    scope: &ScopeId,
    request: ImpactQuery,
) -> anyhow::Result<ImpactResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let id = StableId::new(request.id.clone())?;
    let repo = ctx.repo();
    let store = ctx.live(Live::Canonical).store();
    let graph = walk::ScopeGraph::load(&store, scope, request.include_retired)?;
    let mut seen = BTreeSet::from([id.as_str().to_string()]);
    let mut rules = BTreeSet::new();
    if graph.find(NodeType::Rule, &id).is_some() {
        rules.insert(id.as_str().to_string());
    }
    let mut frontier: Vec<(NodeType, StableId)> = graph
        .kind_of(&id)
        .map(|node_type| vec![(node_type, id)])
        .unwrap_or_default();
    for _ in 0..TRACE_MAX_DEPTH {
        let mut next = Vec::new();
        for (origin_type, origin) in &frontier {
            for step in flow_neighbors(&graph.front(), *origin_type, origin, true) {
                if !seen.insert(step.endpoint.id.as_str().to_string()) {
                    continue;
                }
                if graph
                    .find(step.endpoint.node_type, &step.endpoint.id)
                    .is_none()
                {
                    continue;
                }
                if step.endpoint.node_type == NodeType::Rule {
                    rules.insert(step.endpoint.id.as_str().to_string());
                }
                next.push((step.endpoint.node_type, step.endpoint.id));
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
    let bindings = Bindings::load(&store, scope, request.include_retired)?;
    let scans = ctx.live(Live::ScannedSites).scan_tree()?;
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
