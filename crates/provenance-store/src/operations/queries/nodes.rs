//! Records handed back whole from the kind tables at the snapshot's
//! revision. Each kind read takes its own table handle, so the stamp
//! names every table an answer came from.

use crate::operations::reader::ReadSnapshot;
use provenance_core::model::ProjectionRow;
use provenance_core::protocol::GraphNode;
use provenance_core::{
    Boundary, Domain, NodeType, Question, Requirement, Resolution, Rule, Source, StableId, Topic,
};
use std::collections::BTreeMap;

/// The key a record is filed under: its kind's rank, then its id.
pub(super) type Key = (u8, String);

pub(super) fn key(node_type: NodeType, id: &StableId) -> Key {
    (node_type.rank(), id.as_str().to_string())
}

/// One record that counts under the view: present, and not retired
/// unless retired records are asked for.
pub(super) async fn node(
    snapshot: &ReadSnapshot,
    node_type: NodeType,
    id: &StableId,
    include_retired: bool,
) -> anyhow::Result<Option<GraphNode>> {
    let wanted = [(node_type, id.clone())];
    let mut found = nodes(snapshot, &wanted, include_retired).await?;
    Ok(found.remove(&key(node_type, id)))
}

/// The records behind the given endpoints that count under the view,
/// keyed by kind and id. Each kind is read once, in rank order.
pub(super) async fn nodes(
    snapshot: &ReadSnapshot,
    wanted: &[(NodeType, StableId)],
    include_retired: bool,
) -> anyhow::Result<BTreeMap<Key, GraphNode>> {
    let mut by_kind: BTreeMap<u8, (NodeType, Vec<StableId>)> = BTreeMap::new();
    for (node_type, id) in wanted {
        by_kind
            .entry(node_type.rank())
            .or_insert_with(|| (*node_type, Vec::new()))
            .1
            .push(id.clone());
    }
    let mut found = BTreeMap::new();
    for (node_type, ids) in by_kind.into_values() {
        let records = match node_type {
            NodeType::Source => {
                read::<Source>(snapshot, &ids, include_retired, |r| {
                    GraphNode::Source(Box::new(r))
                })
                .await?
            }
            NodeType::Requirement => {
                read::<Requirement>(snapshot, &ids, include_retired, |r| {
                    GraphNode::Requirement(Box::new(r))
                })
                .await?
            }
            NodeType::Resolution => {
                read::<Resolution>(snapshot, &ids, include_retired, |r| {
                    GraphNode::Resolution(Box::new(r))
                })
                .await?
            }
            NodeType::Rule => {
                read::<Rule>(snapshot, &ids, include_retired, |r| {
                    GraphNode::Rule(Box::new(r))
                })
                .await?
            }
            NodeType::Topic => {
                read::<Topic>(snapshot, &ids, include_retired, |r| {
                    GraphNode::Topic(Box::new(r))
                })
                .await?
            }
            NodeType::Question => {
                read::<Question>(snapshot, &ids, include_retired, |r| {
                    GraphNode::Question(Box::new(r))
                })
                .await?
            }
            NodeType::Domain => {
                read::<Domain>(snapshot, &ids, include_retired, |r| {
                    GraphNode::Domain(Box::new(r))
                })
                .await?
            }
            NodeType::Boundary => {
                read::<Boundary>(snapshot, &ids, include_retired, |r| {
                    GraphNode::Boundary(Box::new(r))
                })
                .await?
            }
        };
        for record in records {
            found.insert(key(record.node_type(), record.id()), record);
        }
    }
    Ok(found)
}

async fn read<K: ProjectionRow>(
    snapshot: &ReadSnapshot,
    ids: &[StableId],
    include_retired: bool,
    wrap: fn(K) -> GraphNode,
) -> anyhow::Result<Vec<GraphNode>> {
    let records = snapshot.table::<K>().by_ids(ids, include_retired).await?;
    Ok(records.into_iter().map(wrap).collect())
}
