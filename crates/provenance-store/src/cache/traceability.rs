use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use provenance_core::{Edge, EdgeType, NodeType, Requirement, Resolution, Rule, Source};

/// One rule and the chain behind it.
///
/// `edges` holds only the edges the walk actually crossed — the produces edge
/// into the rule, the resolves edge from its resolution, and the source
/// references on the requirements reached. A reader asked about one rule, so
/// handing back every edge in the scope answers a question nobody asked.
#[derive(Debug, serde::Serialize)]
pub struct TraceabilityView {
    pub rule: Rule,
    pub resolutions: Vec<Resolution>,
    pub requirements: Vec<Requirement>,
    pub sources: Vec<Source>,
    pub edges: Vec<Edge>,
}

pub fn trace_rule(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    rule_id: &provenance_core::StableId,
) -> anyhow::Result<TraceabilityView> {
    let store = StateStore::new(layout.clone());
    store.with_repository_publication(|| trace_rule_locked(scope, rule_id, &store))
}

fn trace_rule_locked(
    scope: &provenance_core::ScopeId,
    rule_id: &provenance_core::StableId,
    store: &StateStore,
) -> anyhow::Result<TraceabilityView> {
    let rule = store
        .list_rules(scope)?
        .into_iter()
        .find(|rule| rule.id == *rule_id)
        .ok_or_else(|| anyhow::anyhow!("rule not found"))?;
    let scope_edges: Vec<Edge> = store
        .list_edges()?
        .into_iter()
        .filter(|edge| edge.scope_id == *scope)
        .collect();
    let produces_rule = |edge: &Edge| {
        edge.edge_type == EdgeType::Produces
            && edge.to_type == NodeType::Rule
            && edge.to_id == *rule_id
            && matches!(edge.from_type, NodeType::Resolution | NodeType::Requirement)
    };
    let resolution_ids: Vec<_> = scope_edges
        .iter()
        .filter(|edge| produces_rule(edge) && edge.from_type == NodeType::Resolution)
        .map(|edge| edge.from_id.clone())
        .collect();
    let resolves_requirement = |edge: &Edge| {
        edge.edge_type == EdgeType::Resolves
            && edge.from_type == NodeType::Resolution
            && resolution_ids.iter().any(|id| id == &edge.from_id)
    };
    // A produces edge points at the rule, so the producer is its `from` end: a
    // requirement recorded as producing the rule directly, or a resolution
    // whose own requirement is one hop further back along its resolves edge.
    // Reading `to_id` off both kinds collected the rule's own id and dropped
    // every direct requirement producer.
    let requirement_ids: Vec<_> = scope_edges
        .iter()
        .filter(|edge| produces_rule(edge) && edge.from_type == NodeType::Requirement)
        .map(|edge| edge.from_id.clone())
        .chain(
            scope_edges
                .iter()
                .filter(|edge| resolves_requirement(edge))
                .map(|edge| edge.to_id.clone()),
        )
        .collect();
    let references_requirement = |edge: &Edge| {
        edge.edge_type == EdgeType::References
            && edge.from_type == NodeType::Source
            && requirement_ids.iter().any(|id| id == &edge.to_id)
    };
    let source_ids: Vec<_> = scope_edges
        .iter()
        .filter(|edge| references_requirement(edge))
        .map(|edge| edge.from_id.clone())
        .collect();
    let edges: Vec<Edge> = scope_edges
        .iter()
        .filter(|edge| {
            produces_rule(edge) || resolves_requirement(edge) || references_requirement(edge)
        })
        .cloned()
        .collect();
    let resolutions = store
        .list_resolutions(scope)?
        .into_iter()
        .filter(|resolution| resolution_ids.iter().any(|id| id == &resolution.id))
        .collect();
    let requirements = store
        .list_requirements(scope)?
        .into_iter()
        .filter(|requirement| requirement_ids.iter().any(|id| id == &requirement.id))
        .collect();
    let sources = store
        .list_sources(scope)?
        .into_iter()
        .filter(|source| source_ids.iter().any(|id| id == &source.id))
        .collect();
    Ok(TraceabilityView {
        rule,
        resolutions,
        requirements,
        sources,
        edges,
    })
}
