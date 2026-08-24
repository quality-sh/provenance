use provenance_core::protocol::{
    ensure_limit, ensure_max_depth, ensure_protocol_version, take_page, Direction, GraphNode,
    Neighbor, NeighborsQuery, NeighborsResult, TraceQuery, TraceResult, TracedNode,
};
use provenance_core::{Edge, EdgeType, NodeType, ScopeId, StableId};
use provenance_store::state_store::StateStore;
use std::collections::BTreeSet;

use super::records;

pub(super) fn scoped_edges(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<Edge>> {
    let mut edges = store
        .list_edges()?
        .into_iter()
        .filter(|edge| edge.scope_id == *scope)
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    Ok(edges)
}

/// One end of an edge, as this walk reads it.
struct Step {
    edge_type: EdgeType,
    direction: Direction,
    node_type: NodeType,
    id: StableId,
}

/// Reads both ends of one edge for a record, in the directions asked for.
fn steps(
    edge: &Edge,
    from: &StableId,
    node_type: Option<NodeType>,
    wanted: Direction,
) -> Vec<Step> {
    let mut steps = Vec::new();
    if wanted.reads_out()
        && edge.from_id == *from
        && node_type.is_none_or(|kind| kind == edge.from_type)
    {
        steps.push(Step {
            edge_type: edge.edge_type,
            direction: Direction::Out,
            node_type: edge.to_type,
            id: edge.to_id.clone(),
        });
    }
    if wanted.reads_in() && edge.to_id == *from && node_type.is_none_or(|kind| kind == edge.to_type)
    {
        steps.push(Step {
            edge_type: edge.edge_type,
            direction: Direction::In,
            node_type: edge.from_type,
            id: edge.from_id.clone(),
        });
    }
    steps
}

fn selected(edge: &Edge, edge_types: &[EdgeType]) -> bool {
    edge_types.is_empty() || edge_types.contains(&edge.edge_type)
}

pub(super) fn neighbors(
    store: &StateStore,
    scope: &ScopeId,
    request: NeighborsQuery,
) -> anyhow::Result<NeighborsResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let id = StableId::new(request.id.clone())?;
    let nodes = records::load(store, scope, request.include_retired)?;
    let mut found = Vec::new();
    for edge in scoped_edges(store, scope)?
        .iter()
        .filter(|edge| selected(edge, &request.edge_types))
    {
        for step in steps(edge, &id, request.node_type, request.direction) {
            if let Some(node) = records::find(&nodes, Some(step.node_type), &step.id) {
                found.push(Neighbor {
                    edge_type: step.edge_type,
                    direction: step.direction,
                    node: node.clone(),
                });
            }
        }
    }
    found.sort_by_key(neighbor_order);
    let (neighbors, has_more) = take_page(found, request.limit);
    Ok(NeighborsResult {
        id: request.id,
        limit: request.limit,
        has_more,
        neighbors,
    })
}

pub(super) fn trace(
    store: &StateStore,
    scope: &ScopeId,
    request: TraceQuery,
) -> anyhow::Result<TraceResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    ensure_max_depth(request.max_depth)?;
    let id = StableId::new(request.id.clone())?;
    let nodes = records::load(store, scope, request.include_retired)?;
    let edges = scoped_edges(store, scope)?
        .into_iter()
        .filter(|edge| selected(edge, &request.edge_types))
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::from([id.as_str().to_string()]);
    let mut frontier = vec![id];
    let mut reached = Vec::new();
    for depth in 1..=request.max_depth {
        let mut next = Vec::new();
        for origin in &frontier {
            for edge in &edges {
                for step in steps(edge, origin, None, request.direction) {
                    if !seen.insert(step.id.as_str().to_string()) {
                        continue;
                    }
                    if let Some(node) = records::find(&nodes, Some(step.node_type), &step.id) {
                        next.push(node.clone());
                    }
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
    let (nodes, has_more) = take_page(reached, request.limit);
    Ok(TraceResult {
        id: request.id,
        max_depth: request.max_depth,
        limit: request.limit,
        has_more,
        nodes,
    })
}

fn neighbor_order(neighbor: &Neighbor) -> (u8, String, u8, u8) {
    (
        records::rank(neighbor.node.node_type()),
        neighbor.node.id().as_str().to_string(),
        edge_rank(neighbor.edge_type),
        direction_rank(neighbor.direction),
    )
}

fn node_order(node: &GraphNode) -> (u8, String) {
    (
        records::rank(node.node_type()),
        node.id().as_str().to_string(),
    )
}

const fn direction_rank(direction: Direction) -> u8 {
    match direction {
        Direction::Out => 0,
        Direction::In => 1,
        Direction::Both => 2,
    }
}

const fn edge_rank(edge_type: EdgeType) -> u8 {
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
