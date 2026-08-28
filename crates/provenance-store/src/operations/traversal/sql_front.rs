//! The `SqlFront`: indexed row lookups served from the projection.
//!
//! The eight served operations consume this front once the projection is
//! fresh. Rows carry full canonical payloads, so a lookup reconstructs the
//! record the projection attests; the narrow columns stay for predicates.

use super::{Step, TraversalSource};
use crate::cache::open_cache;
use crate::layout::ProvenanceLayout;
use provenance_core::protocol::{Direction, GraphNode};
use provenance_core::{Edge, EdgeType, NodeType, ScopeId, StableId};
use sqlx::{Pool, Row, Sqlite};

/// Serves lookups from one scope of the projection database.
pub struct SqlFront {
    pool: Pool<Sqlite>,
    scope: ScopeId,
}

impl SqlFront {
    pub async fn open(layout: &ProvenanceLayout, scope: &ScopeId) -> anyhow::Result<Self> {
        let pool = open_cache(layout).await?;
        Ok(Self {
            pool,
            scope: scope.clone(),
        })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    async fn payload(&self, table: &str, id: &StableId) -> anyhow::Result<Option<String>> {
        let row = sqlx::query(&format!(
            "SELECT payload FROM {table} WHERE scope_id = ? AND id = ?"
        ))
        .bind(self.scope.as_str())
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| row.get::<String, _>("payload")))
    }

    /// Loads every record of one family's payload column for the scope.
    pub async fn payloads(&self, table: &str) -> anyhow::Result<Vec<String>> {
        let rows = sqlx::query(&format!(
            "SELECT payload FROM {table} WHERE scope_id = ? ORDER BY id"
        ))
        .bind(self.scope.as_str())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.get::<String, _>("payload"))
            .collect())
    }

    /// Loads the whole scope's records in the contract order.
    pub async fn nodes(&self, include_retired: bool) -> anyhow::Result<Vec<GraphNode>> {
        let mut nodes = Vec::new();
        for node_type in [
            NodeType::Source,
            NodeType::Domain,
            NodeType::Requirement,
            NodeType::Boundary,
            NodeType::Resolution,
            NodeType::Rule,
            NodeType::Topic,
            NodeType::Question,
        ] {
            for payload in self.payloads(family_table(node_type)).await? {
                let node = payload_node(node_type, &payload)?;
                if include_retired || !node.retired() {
                    nodes.push(node);
                }
            }
        }
        nodes.sort_by_key(super::node_order);
        Ok(nodes)
    }

    async fn scoped_edges(&self) -> anyhow::Result<Vec<Edge>> {
        let rows = sqlx::query("SELECT payload FROM edges WHERE scope_id = ? ORDER BY id")
            .bind(self.scope.as_str())
            .fetch_all(&self.pool)
            .await?;
        let mut edges = Vec::new();
        for row in rows {
            let payload: String = row.get("payload");
            edges.push(serde_json::from_str(&payload)?);
        }
        Ok(edges)
    }
}

impl TraversalSource for SqlFront {
    async fn find(
        &self,
        node_type: NodeType,
        id: &StableId,
        include_retired: bool,
    ) -> anyhow::Result<Option<GraphNode>> {
        let table = family_table(node_type);
        let Some(payload) = self.payload(table, id).await? else {
            return Ok(None);
        };
        let node = payload_node(node_type, &payload)?;
        if !include_retired && node.retired() {
            return Ok(None);
        }
        Ok(Some(node))
    }

    async fn steps(
        &self,
        origin: &StableId,
        wanted: Direction,
        edge_types: &[EdgeType],
    ) -> anyhow::Result<Vec<Step>> {
        let mut steps = Vec::new();
        for edge in self.scoped_edges().await? {
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

const fn family_table(node_type: NodeType) -> &'static str {
    match node_type {
        NodeType::Source => "sources",
        NodeType::Requirement => "requirements",
        NodeType::Resolution => "resolutions",
        NodeType::Rule => "rules",
        NodeType::Topic => "topics",
        NodeType::Question => "questions",
        NodeType::Domain => "domains",
        NodeType::Boundary => "boundaries",
    }
}

fn payload_node(node_type: NodeType, payload: &str) -> anyhow::Result<GraphNode> {
    let node = match node_type {
        NodeType::Source => GraphNode::Source(Box::new(serde_json::from_str(payload)?)),
        NodeType::Requirement => GraphNode::Requirement(Box::new(serde_json::from_str(payload)?)),
        NodeType::Resolution => GraphNode::Resolution(Box::new(serde_json::from_str(payload)?)),
        NodeType::Rule => GraphNode::Rule(Box::new(serde_json::from_str(payload)?)),
        NodeType::Topic => GraphNode::Topic(Box::new(serde_json::from_str(payload)?)),
        NodeType::Question => GraphNode::Question(Box::new(serde_json::from_str(payload)?)),
        NodeType::Domain => GraphNode::Domain(Box::new(serde_json::from_str(payload)?)),
        NodeType::Boundary => GraphNode::Boundary(Box::new(serde_json::from_str(payload)?)),
    };
    Ok(node)
}
