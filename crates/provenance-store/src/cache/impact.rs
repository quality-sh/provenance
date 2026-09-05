use crate::cache::gaps::GraphRecords;
use crate::cache::serde_name;
use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use provenance_core::model::relations::{flow_neighbors, RecordFront};
use provenance_core::{NodeType, StableId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactDirection {
    Upstream,
    Downstream,
}

#[derive(Debug, serde::Serialize)]
pub struct ImpactNode {
    pub node_type: NodeType,
    pub id: String,
    pub hop_distance: u32,
    pub direction: ImpactDirection,
}

#[derive(Debug, serde::Serialize)]
pub struct ImpactView {
    pub origin_type: NodeType,
    pub origin_id: String,
    pub nodes: Vec<ImpactNode>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ImpactOptions {
    pub max_hops: u32,
    pub follow_indirect: bool,
}

/// The relations impact follows only when asked to: a refinement, a
/// dependency, a supersession, or a spawn changes the reading of a record
/// without changing what it produces. `contradicts` is not here: it has
/// no flow, so the walk never yields it.
const INDIRECT: [&str; 4] = ["refines", "depends_on", "supersedes", "spawned_by"];

pub fn analyze_impact(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    origin_type: NodeType,
    origin_id: &StableId,
    options: ImpactOptions,
) -> anyhow::Result<ImpactView> {
    let store = StateStore::new(layout.clone());
    let records = store.with_repository_publication(|| GraphRecords::load(scope, &store))?;
    let graph = records.graph(scope);
    let front = graph.front();
    let mut nodes = BTreeMap::<(String, String, &'static str), ImpactNode>::new();
    let mut truncated = false;
    for direction in [ImpactDirection::Downstream, ImpactDirection::Upstream] {
        let mut seen =
            BTreeSet::from([(serde_name(&origin_type)?, origin_id.as_str().to_string())]);
        let mut queue = VecDeque::from([(origin_type, origin_id.clone(), 0_u32)]);
        while let Some((node_type, node_id, hops)) = queue.pop_front() {
            let steps = next_hop(
                &front,
                node_type,
                &node_id,
                direction,
                options.follow_indirect,
            );
            if hops >= options.max_hops {
                truncated |= !steps.is_empty();
                continue;
            }
            for (next_type, next_id) in steps {
                let key = (serde_name(&next_type)?, next_id.as_str().to_string());
                if seen.insert(key.clone()) {
                    let hop_distance = hops + 1;
                    nodes.insert(
                        (key.0, key.1, direction_key(direction)),
                        ImpactNode {
                            node_type: next_type,
                            id: next_id.as_str().to_string(),
                            hop_distance,
                            direction,
                        },
                    );
                    queue.push_back((next_type, next_id, hop_distance));
                }
            }
        }
    }
    Ok(ImpactView {
        origin_type,
        origin_id: origin_id.as_str().to_string(),
        nodes: nodes.into_values().collect(),
        truncated,
    })
}

fn next_hop(
    front: &RecordFront<'_>,
    node_type: NodeType,
    node_id: &StableId,
    direction: ImpactDirection,
    follow_indirect: bool,
) -> Vec<(NodeType, StableId)> {
    flow_neighbors(
        front,
        node_type,
        node_id,
        direction == ImpactDirection::Downstream,
    )
    .into_iter()
    .filter(|node| follow_indirect || !INDIRECT.contains(&node.relation))
    .map(|node| (node.endpoint.node_type, node.endpoint.id))
    .collect()
}

const fn direction_key(direction: ImpactDirection) -> &'static str {
    match direction {
        ImpactDirection::Upstream => "upstream",
        ImpactDirection::Downstream => "downstream",
    }
}

#[cfg(test)]
mod tests {
    use super::INDIRECT;
    use provenance_core::model::relations::{declared_relations, RelationFlow};

    /// A name `flow_neighbors` never yields cannot be an indirect step;
    /// an entry for it would be dead.
    #[test]
    fn an_indirect_name_is_a_declared_relation_with_a_flow() {
        for name in INDIRECT {
            assert!(
                declared_relations()
                    .iter()
                    .flat_map(|table| table.iter())
                    .any(|decl| decl.name == name && decl.flow != RelationFlow::None),
                "{name} has no declaration with a flow"
            );
        }
    }
}
