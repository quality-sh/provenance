//! `neighbors` and `trace` over the derived `relations` table: one fetched
//! hop per depth, the core's `related_nodes` order over the rows, and the
//! records handed back whole from their kind tables.

use crate::operations::reader::{kind_of, ReadContext, ReadSnapshot, SqlFront};
use provenance_core::model::relations::{related_nodes, RelatedNode, RelationDirection};
use provenance_core::protocol::{
    Direction, GraphNode, Neighbor, NeighborsQuery, NeighborsResult, TraceQuery, TraceResult,
    TracedNode,
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

/// A missing origin has no fields to follow. Present records that name
/// it still answer.
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

/// Whether the origin exists and has fields to follow.
async fn follows_out(
    snapshot: &ReadSnapshot,
    node_type: NodeType,
    id: &StableId,
) -> anyhow::Result<bool> {
    Ok(nodes::node(snapshot, node_type, id).await?.is_some())
}

/// The origin kind: the one named, or the first kind in rank order that
/// holds a record that counts.
async fn origin_kind(
    snapshot: &ReadSnapshot,
    node_type: Option<NodeType>,
    id: &StableId,
) -> anyhow::Result<Option<NodeType>> {
    match node_type {
        Some(node_type) => Ok(Some(node_type)),
        None => kind_of(snapshot, id).await,
    }
}

/// The records behind the steps that count, in step order, bounded by
/// the page. Each kind group is probed by id first, so only the first
/// `limit` present records decode, and the flag says whether a further
/// record exists past the page; no off-page record is read whole.
async fn hydrate(
    snapshot: &ReadSnapshot,
    steps: &[RelatedNode],

    limit: usize,
) -> anyhow::Result<(Vec<(RelatedNode, GraphNode)>, bool)> {
    let mut selected: Vec<RelatedNode> = Vec::new();
    let mut more = false;
    let mut start = 0;
    while start < steps.len() {
        let kind = steps[start].endpoint.node_type;
        let end = steps[start..]
            .iter()
            .position(|step| step.endpoint.node_type != kind)
            .map_or(steps.len(), |offset| start + offset);
        let wanted: Vec<(NodeType, StableId)> = steps[start..end]
            .iter()
            .map(|step| (kind, step.endpoint.id.clone()))
            .collect();
        let counting: BTreeSet<Key> = nodes::counting(snapshot, &wanted)
            .await?
            .into_iter()
            .map(|(node_type, id)| nodes::key(node_type, &id))
            .collect();
        for step in &steps[start..end] {
            if !counting.contains(&nodes::key(kind, &step.endpoint.id)) {
                continue;
            }
            if selected.len() == limit {
                more = true;
                break;
            }
            selected.push(step.clone());
        }
        if more {
            break;
        }
        start = end;
    }
    let wanted: Vec<(NodeType, StableId)> = selected
        .iter()
        .map(|step| (step.endpoint.node_type, step.endpoint.id.clone()))
        .collect();
    let records = nodes::nodes(snapshot, &wanted).await?;
    let found = selected
        .into_iter()
        .filter_map(|step| {
            let node = records
                .get(&nodes::key(step.endpoint.node_type, &step.endpoint.id))?
                .clone();
            Some((step, node))
        })
        .collect();
    Ok((found, more))
}

/// The neighbours of one record of a known kind, in served order, up to
/// the limit, with whether more neighbours exist.
async fn around(
    snapshot: &ReadSnapshot,
    node_type: NodeType,
    id: &StableId,
    request: &NeighborsQuery,
) -> anyhow::Result<(Vec<Neighbor>, bool)> {
    let follows_out = follows_out(snapshot, node_type, id).await?;
    let front = SqlFront::hop(&snapshot.relations(), &[(node_type, id.clone())]).await?;
    let steps = steps(
        &front,
        node_type,
        id,
        request.direction,
        &request.relations,
        follows_out,
    );
    let (found, more) = hydrate(snapshot, &steps, request.limit).await?;
    let neighbors = found
        .into_iter()
        .map(|(step, node)| Neighbor {
            relation: step.relation.to_string(),
            direction: direction_of(step.direction),
            node,
        })
        .collect();
    Ok((neighbors, more))
}

pub(super) async fn neighbors(
    ctx: &ReadContext,
    request: NeighborsQuery,
) -> anyhow::Result<NeighborsResult> {
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let id = StableId::new(request.id.clone())?;
    let snapshot = ctx.snapshot();
    let (neighbors, has_more) = match origin_kind(snapshot, request.node_type, &id).await? {
        Some(node_type) => around(snapshot, node_type, &id, &request).await?,
        None => (Vec::new(), false),
    };
    Ok(NeighborsResult {
        id: request.id,
        limit: request.limit,
        has_more,
        neighbors,
    })
}

pub(super) async fn trace(ctx: &ReadContext, request: TraceQuery) -> anyhow::Result<TraceResult> {
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let id = StableId::new(request.id.clone())?;
    let snapshot = ctx.snapshot();
    let mut frontier: Vec<(NodeType, StableId)> = origin_kind(snapshot, request.node_type, &id)
        .await?
        .map(|node_type| vec![(node_type, id.clone())])
        .unwrap_or_default();
    // Every later frontier is made of records that count; only the origin
    // can be one whose fields are not followed.
    let mut follows_out = match frontier.first() {
        Some((node_type, id)) => self::follows_out(snapshot, *node_type, id).await?,
        None => true,
    };
    let mut seen: BTreeSet<Key> = frontier
        .iter()
        .map(|(node_type, id)| nodes::key(*node_type, id))
        .collect();
    let mut reached: Vec<TracedNode> = Vec::new();
    let mut more = false;
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
                // path to a missing record is skipped too.
                if seen.insert(nodes::key(step.endpoint.node_type, &step.endpoint.id)) {
                    candidates.push((step.endpoint.node_type, step.endpoint.id));
                }
            }
        }
        follows_out = true;
        // The counting endpoints come back keyed by rank and id, which is
        // the served depth order. Only the page slice decodes; the whole
        // counted level carries the walk forward, so the requested depth
        // bounds the exploration, not the output page.
        let counting = nodes::counting(snapshot, &candidates).await?;
        if counting.is_empty() {
            break;
        }
        let take = (request.limit - reached.len()).min(counting.len());
        more = more || counting.len() > take;
        let page = nodes::nodes(snapshot, &counting[..take]).await?;
        for node in page.into_values() {
            reached.push(TracedNode { depth, node });
        }
        frontier = counting;
        if more {
            break;
        }
    }
    Ok(TraceResult {
        id: request.id,
        max_depth: request.max_depth,
        limit: request.limit,
        has_more: more,
        nodes: reached,
    })
}
