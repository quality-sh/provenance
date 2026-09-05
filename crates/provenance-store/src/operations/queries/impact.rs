use crate::operations::reader::{Live, ReadContext};
use crate::state_store::StateStore;
use provenance_core::model::relations::{flow_neighbors, RecordFront};
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, GraphNode, ImpactQuery, ImpactResult,
    TRACE_MAX_DEPTH,
};
use provenance_core::{NodeType, ScopeId, StableId};
use std::collections::BTreeSet;

use super::super::sites;
use super::{bindings::Bindings, records};

/// The records of one scope as a traversal front, with the loaded node
/// list beside them so a reached record can be handed back whole.
struct ScopeGraph {
    nodes: Vec<GraphNode>,
    sources: Vec<provenance_core::Source>,
    requirements: Vec<provenance_core::Requirement>,
    resolutions: Vec<provenance_core::Resolution>,
    rules: Vec<provenance_core::Rule>,
    topics: Vec<provenance_core::Topic>,
    questions: Vec<provenance_core::Question>,
    domains: Vec<provenance_core::Domain>,
    boundaries: Vec<provenance_core::Boundary>,
}

impl ScopeGraph {
    fn load(store: &StateStore, scope: &ScopeId, include_retired: bool) -> anyhow::Result<Self> {
        let nodes = records::load(store, scope, include_retired)?;
        let mut graph = Self {
            nodes: Vec::new(),
            sources: Vec::new(),
            requirements: Vec::new(),
            resolutions: Vec::new(),
            rules: Vec::new(),
            topics: Vec::new(),
            questions: Vec::new(),
            domains: Vec::new(),
            boundaries: Vec::new(),
        };
        for node in &nodes {
            match node {
                GraphNode::Source(record) => graph.sources.push((**record).clone()),
                GraphNode::Requirement(record) => graph.requirements.push((**record).clone()),
                GraphNode::Resolution(record) => graph.resolutions.push((**record).clone()),
                GraphNode::Rule(record) => graph.rules.push((**record).clone()),
                GraphNode::Topic(record) => graph.topics.push((**record).clone()),
                GraphNode::Question(record) => graph.questions.push((**record).clone()),
                GraphNode::Domain(record) => graph.domains.push((**record).clone()),
                GraphNode::Boundary(record) => graph.boundaries.push((**record).clone()),
            }
        }
        graph.nodes = nodes;
        Ok(graph)
    }

    fn front(&self) -> RecordFront<'_> {
        RecordFront {
            sources: &self.sources,
            requirements: &self.requirements,
            resolutions: &self.resolutions,
            rules: &self.rules,
            topics: &self.topics,
            questions: &self.questions,
            domains: &self.domains,
            boundaries: &self.boundaries,
        }
    }

    fn find(&self, node_type: NodeType, id: &StableId) -> Option<&GraphNode> {
        records::find(&self.nodes, Some(node_type), id)
    }

    fn kind_of(&self, id: &StableId) -> Option<NodeType> {
        records::find(&self.nodes, None, id).map(GraphNode::node_type)
    }
}

fn seen_key((node_type, id): &(NodeType, StableId)) -> (u8, String) {
    (records::rank(*node_type), id.as_str().to_string())
}

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
    let graph = ScopeGraph::load(&store, scope, request.include_retired)?;
    let mut rules = BTreeSet::new();
    if graph.find(NodeType::Rule, &id).is_some() {
        rules.insert(id.as_str().to_string());
    }
    let mut frontier: Vec<(NodeType, StableId)> = graph
        .kind_of(&id)
        .map(|node_type| vec![(node_type, id)])
        .unwrap_or_default();
    let mut seen: BTreeSet<(u8, String)> = frontier.iter().map(seen_key).collect();
    for _ in 0..TRACE_MAX_DEPTH {
        let mut next = Vec::new();
        for (origin_type, origin) in &frontier {
            for step in flow_neighbors(&graph.front(), *origin_type, origin, true) {
                if !seen.insert(seen_key(&(
                    step.endpoint.node_type,
                    step.endpoint.id.clone(),
                ))) {
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
