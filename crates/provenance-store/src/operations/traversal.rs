//! The one Relation traversal core with its two provider fronts.
//!
//! `TraversalSource` is the provider seam: a front answers record lookups
//! and edge steps, and every traversal (neighbors, trace, impact) runs on
//! top of it. Exactly two fronts exist: `RecordFront` serves in-memory
//! record vectors (what `GraphRecords` builds), and `SqlFront` serves
//! indexed row lookups from the projection database. The ordering contract
//! is pinned here once - node rank, then canonical id - so every front
//! answers in the same order by construction.

use crate::state_store::StateStore;
use provenance_core::protocol::{
    ensure_limit, take_page, Direction, GraphNode, Neighbor, NeighborsQuery, NeighborsResult,
    TraceQuery, TracedNode,
};
use provenance_core::{Edge, EdgeType, NodeType, ScopeId, StableId};
pub use sql_front::SqlFront;
use std::collections::BTreeSet;

/// One end of a relation, as a walk reads it.
#[derive(Debug, Clone)]
pub struct Step {
    pub edge_type: EdgeType,
    pub direction: Direction,
    pub node_type: NodeType,
    pub id: StableId,
}

/// The provider seam every traversal runs over.
pub trait TraversalSource: Sync {
    /// Finds one record, honoring the active-view filter.
    fn find(
        &self,
        node_type: NodeType,
        id: &StableId,
        include_retired: bool,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<GraphNode>>> + Send;

    /// Reads both ends of the relations touching one record, in the
    /// directions and edge families asked for.
    fn steps(
        &self,
        origin: &StableId,
        wanted: Direction,
        edge_types: &[EdgeType],
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<Step>>> + Send;
}

/// The record vectors a front is built from, in one settled order.
pub struct RecordFront {
    nodes: Vec<GraphNode>,
    edges: Vec<Edge>,
}

impl RecordFront {
    /// Loads one scope's graph through the canonical readers.
    pub fn load(
        store: &StateStore,
        scope: &ScopeId,
        include_retired: bool,
    ) -> anyhow::Result<Self> {
        let mut nodes = Vec::new();
        nodes.extend(
            store
                .list_sources(scope)?
                .into_iter()
                .map(|record| GraphNode::Source(Box::new(record))),
        );
        nodes.extend(
            store
                .list_domains(scope)?
                .into_iter()
                .map(|record| GraphNode::Domain(Box::new(record))),
        );
        nodes.extend(
            store
                .list_requirements(scope)?
                .into_iter()
                .map(|record| GraphNode::Requirement(Box::new(record))),
        );
        nodes.extend(
            store
                .list_boundaries(scope)?
                .into_iter()
                .map(|record| GraphNode::Boundary(Box::new(record))),
        );
        nodes.extend(
            store
                .list_resolutions(scope)?
                .into_iter()
                .map(|record| GraphNode::Resolution(Box::new(record))),
        );
        nodes.extend(
            store
                .list_rules(scope)?
                .into_iter()
                .map(|record| GraphNode::Rule(Box::new(record))),
        );
        nodes.extend(
            store
                .list_topics(scope)?
                .into_iter()
                .map(|record| GraphNode::Topic(Box::new(record))),
        );
        nodes.extend(
            store
                .list_questions(scope)?
                .into_iter()
                .map(|record| GraphNode::Question(Box::new(record))),
        );
        nodes.retain(|node| include_retired || !node.retired());
        nodes.sort_by(|left, right| {
            rank(left.node_type())
                .cmp(&rank(right.node_type()))
                .then_with(|| left.id().as_str().cmp(right.id().as_str()))
        });
        let mut edges: Vec<Edge> = store
            .list_edges()?
            .into_iter()
            .filter(|edge| edge.scope_id == *scope)
            .collect();
        edges.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(Self { nodes, edges })
    }

    /// Finds one record in the loaded vectors.
    pub fn find_blocking(
        &self,
        node_type: NodeType,
        id: &StableId,
        include_retired: bool,
    ) -> anyhow::Result<Option<GraphNode>> {
        Ok(self
            .nodes
            .iter()
            .find(|node| {
                node.id() == id
                    && node.node_type() == node_type
                    && (include_retired || !node.retired())
            })
            .cloned())
    }
}

impl TraversalSource for RecordFront {
    async fn find(
        &self,
        node_type: NodeType,
        id: &StableId,
        include_retired: bool,
    ) -> anyhow::Result<Option<GraphNode>> {
        self.find_blocking(node_type, id, include_retired)
    }

    async fn steps(
        &self,
        origin: &StableId,
        wanted: Direction,
        edge_types: &[EdgeType],
    ) -> anyhow::Result<Vec<Step>> {
        let mut steps = Vec::new();
        for edge in &self.edges {
            if !edge_types.is_empty() && !edge_types.contains(&edge.edge_type) {
                continue;
            }
            if wanted.reads_out() && edge.from_id == *origin {
                steps.push(Step {
                    edge_type: edge.edge_type,
                    direction: Direction::Out,
                    node_type: edge.to_type,
                    id: edge.to_id.clone(),
                });
            }
            if wanted.reads_in() && edge.to_id == *origin {
                steps.push(Step {
                    edge_type: edge.edge_type,
                    direction: Direction::In,
                    node_type: edge.from_type,
                    id: edge.from_id.clone(),
                });
            }
        }
        Ok(steps)
    }
}

/// Reads every record one relation away from a record, in contract order.
///
/// Pagination belongs to the served executor, which owns cursors; the
/// core owns only the ordering contract.
pub async fn neighbors_raw<S: TraversalSource>(
    source: &S,
    request: NeighborsQuery,
) -> anyhow::Result<Vec<Neighbor>> {
    let id = StableId::new(request.id.clone())?;
    let mut found = Vec::new();
    for step in source
        .steps(&id, request.direction, &request.edge_types)
        .await?
    {
        if request
            .node_type
            .is_some_and(|wanted| wanted != step.node_type)
        {
            continue;
        }
        if let Some(node) = source
            .find(step.node_type, &step.id, request.include_retired)
            .await?
        {
            found.push(Neighbor {
                edge_type: step.edge_type,
                direction: step.direction,
                node,
            });
        }
    }
    found.sort_by_key(neighbor_order);
    Ok(found)
}

/// Builds the unpaginated neighbors answer (used by the differential
/// harness against the original executor).
pub async fn neighbors<S: TraversalSource>(
    source: &S,
    request: NeighborsQuery,
) -> anyhow::Result<NeighborsResult> {
    provenance_core::protocol::ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let neighbors = neighbors_raw(source, request_without_cursor(&request)).await?;
    let (neighbors, has_more) = take_page(neighbors, request.limit);
    Ok(NeighborsResult {
        stamp: None,
        id: request.id,
        limit: request.limit,
        has_more,
        neighbors,
        next_cursor: None,
    })
}

fn request_without_cursor(request: &NeighborsQuery) -> NeighborsQuery {
    let mut copy = request.clone();
    copy.cursor = None;
    copy
}

/// Walks outward from a record for a bounded number of hops, unpaginated.
pub async fn trace_raw<S: TraversalSource>(
    source: &S,
    request: TraceQuery,
) -> anyhow::Result<Vec<TracedNode>> {
    let id = StableId::new(request.id.clone())?;
    let mut seen = BTreeSet::from([id.as_str().to_string()]);
    let mut frontier = vec![id];
    let mut reached = Vec::new();
    for depth in 1..=request.max_depth {
        let mut next = Vec::new();
        for origin in &frontier {
            for step in source
                .steps(origin, request.direction, &request.edge_types)
                .await?
            {
                if !seen.insert(step.id.as_str().to_string()) {
                    continue;
                }
                if let Some(node) = source
                    .find(step.node_type, &step.id, request.include_retired)
                    .await?
                {
                    next.push(node);
                }
            }
        }
        next.sort_by_key(node_order);
        if next.is_empty() {
            break;
        }
        frontier = next.iter().map(|node| node.id().clone()).collect();
        reached.extend(next.into_iter().map(|node| TracedNode { depth, node }));
        if reached.len() > request.limit {
            break;
        }
    }
    Ok(reached)
}

/// Fixes the order node types are read in: the contract rank order, with
/// Domain and Boundary in the appended slots.
pub const fn rank(node_type: NodeType) -> u8 {
    match node_type {
        NodeType::Source => 0,
        NodeType::Requirement => 1,
        NodeType::Resolution => 2,
        NodeType::Rule => 3,
        NodeType::Topic => 4,
        NodeType::Question => 5,
        NodeType::Domain => 6,
        NodeType::Boundary => 7,
    }
}

pub(crate) fn neighbor_order(neighbor: &Neighbor) -> (u8, String, u8, u8) {
    (
        rank(neighbor.node.node_type()),
        neighbor.node.id().as_str().to_string(),
        edge_rank(neighbor.edge_type),
        direction_rank(neighbor.direction),
    )
}

pub(crate) fn node_order(node: &GraphNode) -> (u8, String) {
    (rank(node.node_type()), node.id().as_str().to_string())
}

pub(crate) const fn direction_rank(direction: Direction) -> u8 {
    match direction {
        Direction::Out => 0,
        Direction::In => 1,
        Direction::Both => 2,
    }
}

mod sql_front;

pub(crate) const fn edge_rank(edge_type: EdgeType) -> u8 {
    match edge_type {
        EdgeType::References => 0,
        EdgeType::RefinesInto => 1,
        EdgeType::DependsOn => 2,
        EdgeType::Contradicts => 3,
        EdgeType::Supersedes => 4,
        EdgeType::Needs => 5,
        EdgeType::Resolves => 6,
        EdgeType::Spawns => 7,
        EdgeType::Produces => 8,
    }
}
