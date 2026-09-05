use crate::operations::reader::{Live, ReadContext};
use crate::state_store::StateStore;
use provenance_core::model::relations::{
    related_nodes, RecordFront, RelatedNode, RelationDirection,
};
use provenance_core::protocol::{
    ensure_limit, ensure_max_depth, ensure_protocol_version, take_page, Direction, GraphNode,
    Neighbor, NeighborsQuery, NeighborsResult, TraceQuery, TraceResult, TracedNode,
};
use provenance_core::{NodeType, ScopeId, StableId};
use std::collections::BTreeSet;

use super::records;

/// The records of one scope as a traversal front, with the loaded node
/// list beside them so a reached record can be handed back whole.
pub(super) struct ScopeGraph {
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
    pub(super) fn load(
        store: &StateStore,
        scope: &ScopeId,
        include_retired: bool,
    ) -> anyhow::Result<Self> {
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

    pub(super) fn front(&self) -> RecordFront<'_> {
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

    pub(super) fn find(&self, node_type: NodeType, id: &StableId) -> Option<&GraphNode> {
        records::find(&self.nodes, Some(node_type), id)
    }

    /// The kind of a record named without one: the first kind, in rank
    /// order, that holds the id.
    pub(super) fn kind_of(&self, id: &StableId) -> Option<NodeType> {
        records::find(&self.nodes, None, id).map(GraphNode::node_type)
    }

    /// The relations around one record that the request admits.
    pub(super) fn steps(
        &self,
        node_type: NodeType,
        id: &StableId,
        wanted: Direction,
        relations: &[String],
    ) -> Vec<RelatedNode> {
        related_nodes(&self.front(), node_type, id)
            .into_iter()
            .filter(|node| match node.direction {
                RelationDirection::Out => wanted.reads_out(),
                RelationDirection::In => wanted.reads_in(),
            })
            .filter(|node| {
                relations.is_empty() || relations.iter().any(|name| name == node.relation)
            })
            .collect()
    }
}

const fn direction_of(direction: RelationDirection) -> Direction {
    match direction {
        RelationDirection::Out => Direction::Out,
        RelationDirection::In => Direction::In,
    }
}

pub(super) fn neighbors(
    ctx: &ReadContext,
    scope: &ScopeId,
    request: NeighborsQuery,
) -> anyhow::Result<NeighborsResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    ensure_relations(&request.relations)?;
    let id = StableId::new(request.id.clone())?;
    let store = ctx.live(Live::Canonical).store();
    let graph = ScopeGraph::load(&store, scope, request.include_retired)?;
    let mut found = Vec::new();
    if let Some(node_type) = request.node_type.or_else(|| graph.kind_of(&id)) {
        for step in graph.steps(node_type, &id, request.direction, &request.relations) {
            if let Some(node) = graph.find(step.endpoint.node_type, &step.endpoint.id) {
                found.push(Neighbor {
                    relation: step.relation.to_string(),
                    direction: direction_of(step.direction),
                    node: node.clone(),
                });
            }
        }
    }
    let (neighbors, has_more) = take_page(found, request.limit);
    Ok(NeighborsResult {
        id: request.id,
        limit: request.limit,
        has_more,
        neighbors,
    })
}

pub(super) fn trace(
    ctx: &ReadContext,
    scope: &ScopeId,
    request: TraceQuery,
) -> anyhow::Result<TraceResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    ensure_max_depth(request.max_depth)?;
    ensure_relations(&request.relations)?;
    let id = StableId::new(request.id.clone())?;
    let store = ctx.live(Live::Canonical).store();
    let graph = ScopeGraph::load(&store, scope, request.include_retired)?;
    let mut frontier: Vec<(NodeType, StableId)> = request
        .node_type
        .or_else(|| graph.kind_of(&id))
        .map(|node_type| vec![(node_type, id)])
        .unwrap_or_default();
    let mut seen: BTreeSet<(u8, String)> = frontier.iter().map(seen_key).collect();
    let mut reached = Vec::new();
    for depth in 1..=request.max_depth {
        let mut next = Vec::new();
        for (origin_type, origin) in &frontier {
            for step in graph.steps(*origin_type, origin, request.direction, &request.relations) {
                if !seen.insert(seen_key(&(
                    step.endpoint.node_type,
                    step.endpoint.id.clone(),
                ))) {
                    continue;
                }
                if let Some(node) = graph.find(step.endpoint.node_type, &step.endpoint.id) {
                    next.push(node.clone());
                }
            }
        }
        next.sort_by_key(node_order);
        if next.is_empty() {
            break;
        }
        frontier = next
            .iter()
            .map(|node| (node.node_type(), node.id().clone()))
            .collect();
        reached.extend(next.into_iter().map(|node| TracedNode { depth, node }));
        if reached.len() > request.limit {
            break;
        }
    }
    let (nodes, has_more) = take_page(reached, request.limit);
    Ok(TraceResult {
        id: request.id,
        max_depth: request.max_depth,
        limit: request.limit,
        has_more,
        nodes,
    })
}

/// Refuses a filter naming a relation no declaration carries.
fn ensure_relations(relations: &[String]) -> anyhow::Result<()> {
    for name in relations {
        if !provenance_core::model::relations::is_relation_name(name) {
            anyhow::bail!(
                "{}",
                provenance_core::model::relations::unknown_relation_refusal(name)
            );
        }
    }
    Ok(())
}

/// The key a walk remembers a visited record by: its kind and its id,
/// so one id under two kinds is two records.
pub(super) fn seen_key((node_type, id): &(NodeType, StableId)) -> (u8, String) {
    (records::rank(*node_type), id.as_str().to_string())
}

fn node_order(node: &GraphNode) -> (u8, String) {
    (
        records::rank(node.node_type()),
        node.id().as_str().to_string(),
    )
}
