//! `neighbors` and `trace` over the derived `relations` table: one fetched
//! hop per depth, the core's `related_nodes` order over the rows, and the
//! records handed back whole from their kind tables.

use crate::operations::reader::{kind_of, ReadContext, ReadSnapshot, SqlFront};
use provenance_core::model::relations::{related_nodes, RelatedNode, RelationDirection};
use provenance_core::protocol::{
    ensure_limit, ensure_max_depth, ensure_protocol_version, take_page, Direction, GraphNode,
    Neighbor, NeighborsQuery, NeighborsResult, TraceQuery, TraceResult, TracedNode,
};
use provenance_core::{NodeType, StableId};
use std::collections::BTreeSet;

use super::nodes::{self, Key};

const fn direction_of(direction: RelationDirection) -> Direction {
    match direction {
        RelationDirection::Out => Direction::Out,
        RelationDirection::In => Direction::In,
    }
}

/// The relations around one record that the request admits. A retired
/// origin in an active view has no fields to follow, so its out rows are
/// dropped; the live records that name it still answer.
fn steps(
    front: &SqlFront,
    node_type: NodeType,
    id: &StableId,
    wanted: Direction,
    relations: &[String],
    follows_out: bool,
) -> Vec<RelatedNode> {
    related_nodes(front, node_type, id)
        .into_iter()
        .filter(|node| match node.direction {
            RelationDirection::Out => follows_out && wanted.reads_out(),
            RelationDirection::In => wanted.reads_in(),
        })
        .filter(|node| relations.is_empty() || relations.iter().any(|name| name == node.relation))
        .collect()
}

/// Whether the origin's own fields are followed: always when the view
/// includes retired records, otherwise only for a record that counts.
async fn follows_out(
    snapshot: &ReadSnapshot,
    node_type: NodeType,
    id: &StableId,
    include_retired: bool,
) -> anyhow::Result<bool> {
    if include_retired {
        return Ok(true);
    }
    Ok(nodes::node(snapshot, node_type, id, false).await?.is_some())
}

/// The origin kind: the one named, or the first kind in rank order that
/// holds a record that counts.
async fn origin_kind(
    snapshot: &ReadSnapshot,
    node_type: Option<NodeType>,
    id: &StableId,
    include_retired: bool,
) -> anyhow::Result<Option<NodeType>> {
    match node_type {
        Some(node_type) => Ok(Some(node_type)),
        None => kind_of(snapshot, id, include_retired).await,
    }
}

/// The records behind the steps that count, in step order. The steps
/// arrive sorted by kind rank, so each kind is read once, and the read
/// stops once `limit + 1` records are in hand: the page and its cut flag
/// are decided, and no later table is touched.
async fn hydrate(
    snapshot: &ReadSnapshot,
    steps: &[RelatedNode],
    include_retired: bool,
    limit: usize,
) -> anyhow::Result<Vec<(RelatedNode, GraphNode)>> {
    let mut found = Vec::new();
    let mut start = 0;
    while start < steps.len() && found.len() <= limit {
        let kind = steps[start].endpoint.node_type;
        let end = steps[start..]
            .iter()
            .position(|step| step.endpoint.node_type != kind)
            .map_or(steps.len(), |offset| start + offset);
        let wanted: Vec<(NodeType, StableId)> = steps[start..end]
            .iter()
            .map(|step| (kind, step.endpoint.id.clone()))
            .collect();
        let records = nodes::nodes(snapshot, &wanted, include_retired).await?;
        for step in &steps[start..end] {
            if let Some(node) = records.get(&nodes::key(kind, &step.endpoint.id)) {
                found.push((step.clone(), node.clone()));
            }
        }
        start = end;
    }
    Ok(found)
}

/// The neighbours of one record of a known kind, in served order, up to
/// one past the limit.
async fn around(
    snapshot: &ReadSnapshot,
    node_type: NodeType,
    id: &StableId,
    request: &NeighborsQuery,
) -> anyhow::Result<Vec<Neighbor>> {
    let follows_out = follows_out(snapshot, node_type, id, request.include_retired).await?;
    let front = SqlFront::hop(&snapshot.relations(), &[(node_type, id.clone())]).await?;
    let steps = steps(
        &front,
        node_type,
        id,
        request.direction,
        &request.relations,
        follows_out,
    );
    Ok(
        hydrate(snapshot, &steps, request.include_retired, request.limit)
            .await?
            .into_iter()
            .map(|(step, node)| Neighbor {
                relation: step.relation.to_string(),
                direction: direction_of(step.direction),
                node,
            })
            .collect(),
    )
}

pub(super) async fn neighbors(
    ctx: &ReadContext,
    request: NeighborsQuery,
) -> anyhow::Result<NeighborsResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    ensure_relations(&request.relations)?;
    let id = StableId::new(request.id.clone())?;
    let snapshot = ctx.snapshot();
    let found = match origin_kind(snapshot, request.node_type, &id, request.include_retired).await?
    {
        Some(node_type) => around(snapshot, node_type, &id, &request).await?,
        None => Vec::new(),
    };
    let (neighbors, has_more) = take_page(found, request.limit);
    Ok(NeighborsResult {
        id: request.id,
        limit: request.limit,
        has_more,
        neighbors,
    })
}

pub(super) async fn trace(ctx: &ReadContext, request: TraceQuery) -> anyhow::Result<TraceResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    ensure_max_depth(request.max_depth)?;
    ensure_relations(&request.relations)?;
    let id = StableId::new(request.id.clone())?;
    let snapshot = ctx.snapshot();
    let include_retired = request.include_retired;
    let mut frontier: Vec<(NodeType, StableId)> =
        origin_kind(snapshot, request.node_type, &id, include_retired)
            .await?
            .map(|node_type| vec![(node_type, id.clone())])
            .unwrap_or_default();
    // Every later frontier is made of records that count; only the origin
    // can be one whose fields are not followed.
    let mut follows_out = match frontier.first() {
        Some((node_type, id)) => {
            self::follows_out(snapshot, *node_type, id, include_retired).await?
        }
        None => true,
    };
    let mut seen: BTreeSet<Key> = frontier
        .iter()
        .map(|(node_type, id)| nodes::key(*node_type, id))
        .collect();
    let mut reached = Vec::new();
    for depth in 1..=request.max_depth {
        if frontier.is_empty() {
            break;
        }
        let front = SqlFront::hop(&snapshot.relations(), &frontier).await?;
        let mut candidates: Vec<(NodeType, StableId)> = Vec::new();
        for (origin_type, origin) in &frontier {
            for step in steps(
                &front,
                *origin_type,
                origin,
                request.direction,
                &request.relations,
                follows_out,
            ) {
                // Marked seen before the record is checked, so a second
                // path to a retired or dangling record is skipped too.
                if seen.insert(nodes::key(step.endpoint.node_type, &step.endpoint.id)) {
                    candidates.push((step.endpoint.node_type, step.endpoint.id));
                }
            }
        }
        follows_out = true;
        // The map is keyed by rank and id, which is the served depth order.
        let next: Vec<GraphNode> = nodes::nodes(snapshot, &candidates, include_retired)
            .await?
            .into_values()
            .collect();
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
