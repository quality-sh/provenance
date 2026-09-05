//! Records handed back whole from the kind tables at the snapshot's
//! revision. Each kind read takes its own table handle, so the stamp
//! names every table an answer came from.

use crate::operations::reader::ReadSnapshot;
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

/// Runs one body over the record type behind a node kind, with `$kind`
/// bound to the type and `$wrap` to its `GraphNode` constructor.
macro_rules! for_kind {
    ($node_type:expr, $kind:ident, $wrap:ident => $body:expr) => {
        match $node_type {
            NodeType::Source => {
                type $kind = Source;
                #[allow(unused_variables)]
                let $wrap = |record: Source| GraphNode::Source(Box::new(record));
                $body
            }
            NodeType::Requirement => {
                type $kind = Requirement;
                #[allow(unused_variables)]
                let $wrap = |record: Requirement| GraphNode::Requirement(Box::new(record));
                $body
            }
            NodeType::Resolution => {
                type $kind = Resolution;
                #[allow(unused_variables)]
                let $wrap = |record: Resolution| GraphNode::Resolution(Box::new(record));
                $body
            }
            NodeType::Rule => {
                type $kind = Rule;
                #[allow(unused_variables)]
                let $wrap = |record: Rule| GraphNode::Rule(Box::new(record));
                $body
            }
            NodeType::Topic => {
                type $kind = Topic;
                #[allow(unused_variables)]
                let $wrap = |record: Topic| GraphNode::Topic(Box::new(record));
                $body
            }
            NodeType::Question => {
                type $kind = Question;
                #[allow(unused_variables)]
                let $wrap = |record: Question| GraphNode::Question(Box::new(record));
                $body
            }
            NodeType::Domain => {
                type $kind = Domain;
                #[allow(unused_variables)]
                let $wrap = |record: Domain| GraphNode::Domain(Box::new(record));
                $body
            }
            NodeType::Boundary => {
                type $kind = Boundary;
                #[allow(unused_variables)]
                let $wrap = |record: Boundary| GraphNode::Boundary(Box::new(record));
                $body
            }
        }
    };
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
        let records: Vec<GraphNode> = for_kind!(node_type, K, wrap => {
            snapshot
                .table::<K>()
                .by_ids(&ids, include_retired)
                .await?
                .into_iter()
                .map(wrap)
                .collect()
        });
        for record in records {
            found.insert(key(record.node_type(), record.id()), record);
        }
    }
    Ok(found)
}

/// The endpoints that name a record that counts under the view, in rank
/// and id order. Each kind is asked once, and only for its ids.
pub(super) async fn counting(
    snapshot: &ReadSnapshot,
    wanted: &[(NodeType, StableId)],
    include_retired: bool,
) -> anyhow::Result<Vec<(NodeType, StableId)>> {
    let mut by_kind: BTreeMap<u8, (NodeType, Vec<StableId>)> = BTreeMap::new();
    for (node_type, id) in wanted {
        by_kind
            .entry(node_type.rank())
            .or_insert_with(|| (*node_type, Vec::new()))
            .1
            .push(id.clone());
    }
    let mut found = Vec::new();
    for (node_type, ids) in by_kind.into_values() {
        let counting: Vec<StableId> = for_kind!(node_type, K, wrap => {
            snapshot.table::<K>().ids_that_count(&ids, include_retired).await?
        });
        found.extend(counting.into_iter().map(|id| (node_type, id)));
    }
    Ok(found)
}

/// The records of one kind whose search text holds the needle, in id
/// order, under the view.
pub(super) async fn search(
    snapshot: &ReadSnapshot,
    node_type: NodeType,
    needle: &str,
    include_retired: bool,
) -> anyhow::Result<Vec<GraphNode>> {
    Ok(for_kind!(node_type, K, wrap => {
        snapshot
            .table::<K>()
            .search(needle, include_retired)
            .await?
            .into_iter()
            .map(wrap)
            .collect()
    }))
}
