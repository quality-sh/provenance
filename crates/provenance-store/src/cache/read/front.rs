//! One fetched hop of the `relations` table as a `RelationSource`.
//!
//! The read database is async and the traversal core is not, so the front is fetched
//! first: two indexed queries per owner kind and per chunk of the
//! frontier, out rows over `idx_relations_out` and in rows over
//! `idx_relations_in`. `related_nodes` and `flow_neighbors` then run
//! unchanged over the rows, and the core owns the order and the dedupe.
//! Each relation name is interned to the `&'static str` its declaration
//! carries, so a row naming an undeclared relation refuses the hop.

use crate::cache::serde_name;
use crate::operations::reader::Relations;
use crate::read_database::{SqlColumns, SqlRow, SqlValue};
use provenance_core::model::relations::{declaration_for, RelationEndpoint, RelationSource, LINKS};
use provenance_core::{NodeType, StableId};
use std::collections::{BTreeMap, BTreeSet};

/// A record key the maps can order: node rank, then id.
type Key = (u8, String);

type Rows = Vec<(&'static str, RelationEndpoint)>;

/// The relation rows around one frontier, fetched once.
#[derive(Debug)]
pub struct SqlFront {
    frontier: BTreeSet<Key>,
    outgoing: BTreeMap<Key, Rows>,
    incoming: BTreeMap<Key, Rows>,
}

fn key(node_type: NodeType, id: &str) -> Key {
    (node_type.rank(), id.to_string())
}

/// The declared name behind a stored relation word, on the owner kind
/// that stores it.
fn intern(owner: NodeType, name: &str) -> anyhow::Result<&'static str> {
    if name == LINKS {
        return Ok(LINKS);
    }
    declaration_for(owner, name)
        .map(|decl| decl.name)
        .ok_or_else(|| {
            crate::refusal::fault!(
                "relation row names `{name}` on {owner:?}, which no declaration carries"
            )
        })
}

/// The kinds the frontier holds, each with its ids, in rank order.
fn by_kind(frontier: &[(NodeType, StableId)]) -> BTreeMap<u8, (NodeType, Vec<&str>)> {
    let mut kinds: BTreeMap<u8, (NodeType, Vec<&str>)> = BTreeMap::new();
    for (node_type, id) in frontier {
        kinds
            .entry(node_type.rank())
            .or_insert_with(|| (*node_type, Vec::new()))
            .1
            .push(id.as_str());
    }
    kinds
}

impl SqlFront {
    /// Fetches every relation row that touches the frontier.
    pub async fn hop(
        relations: &Relations<'_>,
        frontier: &[(NodeType, StableId)],
    ) -> anyhow::Result<Self> {
        let mut front = Self {
            frontier: frontier
                .iter()
                .map(|(node_type, id)| key(*node_type, id.as_str()))
                .collect(),
            outgoing: BTreeMap::new(),
            incoming: BTreeMap::new(),
        };
        let snapshot = relations.snapshot();
        for (node_type, ids) in by_kind(frontier).into_values() {
            let word = serde_name(&node_type)?;
            for chunk in ids.chunks(snapshot.list_values()) {
                let marks = vec!["?"; chunk.len()].join(", ");
                let out = format!(
                    "SELECT owner_id, relation, target_type, target_id FROM relations \
                     WHERE scope_id = ? AND owner_type = ? AND owner_id IN ({marks}) \
                     ORDER BY owner_id, relation, target_type, target_id"
                );
                let into = format!(
                    "SELECT target_id, relation, owner_type, owner_id FROM relations \
                     WHERE scope_id = ? AND target_type = ? AND target_id IN ({marks}) \
                     ORDER BY target_id, owner_type, owner_id, relation"
                );
                let values: Vec<SqlValue> = [snapshot.scope_value(), word.as_str().into()]
                    .into_iter()
                    .chain(chunk.iter().map(|id| SqlValue::from(*id)))
                    .collect();
                for (sql, outward, values) in [(out, true, values.clone()), (into, false, values)] {
                    let rows = snapshot.fetch(&sql, values).await?;
                    for row in rows {
                        front.take(node_type, row, outward)?;
                    }
                }
            }
        }
        Ok(front)
    }

    /// Files one fetched row: an out row under its owner, an in row under
    /// its target, the other end as the endpoint.
    fn take(&mut self, kind: NodeType, mut row: SqlRow, outward: bool) -> anyhow::Result<()> {
        // The columns are read in their order; the name is checked as
        // text first and read again after the ids move out of the row.
        let anchor = row.take_text(0)?;
        row.text(1)?;
        let other_type = NodeType::parse(row.text(2)?)?;
        let other_id = StableId::new(row.take_text(3)?)?;
        let owner = if outward { kind } else { other_type };
        let relation = intern(owner, row.text(1)?)?;
        let endpoint = RelationEndpoint {
            node_type: other_type,
            id: other_id,
        };
        let rows = if outward {
            &mut self.outgoing
        } else {
            &mut self.incoming
        };
        rows.entry((kind.rank(), anchor))
            .or_default()
            .push((relation, endpoint));
        Ok(())
    }

    fn rows(&self, side: &BTreeMap<Key, Rows>, node_type: NodeType, id: &StableId) -> Rows {
        let key = key(node_type, id.as_str());
        debug_assert!(
            self.frontier.contains(&key),
            "{node_type:?} {} is outside the fetched frontier",
            id.as_str()
        );
        side.get(&key).cloned().unwrap_or_default()
    }
}

impl RelationSource for SqlFront {
    fn outgoing(&self, node_type: NodeType, id: &StableId) -> Rows {
        self.rows(&self.outgoing, node_type, id)
    }

    fn incoming(&self, node_type: NodeType, id: &StableId) -> Rows {
        self.rows(&self.incoming, node_type, id)
    }
}
