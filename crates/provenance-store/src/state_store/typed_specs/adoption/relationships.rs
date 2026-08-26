use std::collections::BTreeSet;

use provenance_core::{Edge, EdgeType, NodeType, StableId};

use super::super::{rule_address, DesiredTypedIds};
use crate::state_store::{TypedFieldChange, TypedSpecInput};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum RelationshipNode {
    Source,
    Requirement,
    Resolution,
    Rule,
    Topic,
    Question,
}

type Relationship = (RelationshipNode, String, RelationshipNode, String);

pub(super) struct DesiredRelationships {
    references: BTreeSet<Relationship>,
    produces: BTreeSet<Relationship>,
}

impl DesiredRelationships {
    pub(super) fn new(input: &TypedSpecInput, ids: &DesiredTypedIds) -> anyhow::Result<Self> {
        let mut references = BTreeSet::new();
        for requirement in &input.requirements {
            for source in &requirement.sources {
                references.insert(relation(
                    NodeType::Source,
                    &ids.sources[source],
                    NodeType::Requirement,
                    &ids.requirements[&requirement.key],
                ));
            }
        }
        let mut produces = BTreeSet::new();
        for rule in &input.rules {
            let address = rule_address(&input.spec, rule)?;
            for requirement in &rule.requirements {
                produces.insert(relation(
                    NodeType::Requirement,
                    &ids.requirements[requirement],
                    NodeType::Rule,
                    &ids.rules[&address],
                ));
            }
        }
        Ok(Self {
            references,
            produces,
        })
    }

    pub(super) fn source_matches(&self, id: &StableId, edges: &[Edge]) -> bool {
        current_relationships(edges, EdgeType::References, |edge| {
            edge.from_type == NodeType::Source
                && edge.from_id == *id
                && edge.to_type == NodeType::Requirement
        }) == selected(&self.references, |relation| relation.1 == id.as_str())
    }

    pub(super) fn requirement_matches(&self, id: &StableId, edges: &[Edge]) -> bool {
        let mut current = current_relationships(edges, EdgeType::References, |edge| {
            edge.from_type == NodeType::Source
                && edge.to_type == NodeType::Requirement
                && edge.to_id == *id
        });
        current.extend(current_relationships(edges, EdgeType::Produces, |edge| {
            edge.from_type == NodeType::Requirement
                && edge.from_id == *id
                && edge.to_type == NodeType::Rule
        }));
        let mut desired = selected(&self.references, |relation| relation.3 == id.as_str());
        desired.extend(selected(&self.produces, |relation| {
            relation.1 == id.as_str()
        }));
        current == desired
    }

    pub(super) fn rule_matches(&self, id: &StableId, edges: &[Edge]) -> bool {
        current_relationships(edges, EdgeType::Produces, |edge| {
            edge.from_type == NodeType::Requirement
                && edge.to_type == NodeType::Rule
                && edge.to_id == *id
        }) == selected(&self.produces, |relation| relation.3 == id.as_str())
    }

    pub(super) fn add_source_change(
        &self,
        id: &StableId,
        edges: &[Edge],
        changes: &mut Vec<TypedFieldChange>,
    ) {
        Self::add_change(
            current_relationships(edges, EdgeType::References, |edge| {
                edge.from_type == NodeType::Source
                    && edge.from_id == *id
                    && edge.to_type == NodeType::Requirement
            }),
            selected(&self.references, |relation| relation.1 == id.as_str()),
            changes,
        );
    }

    pub(super) fn add_requirement_change(
        &self,
        id: &StableId,
        edges: &[Edge],
        changes: &mut Vec<TypedFieldChange>,
    ) {
        let mut current = current_relationships(edges, EdgeType::References, |edge| {
            edge.from_type == NodeType::Source
                && edge.to_type == NodeType::Requirement
                && edge.to_id == *id
        });
        current.extend(current_relationships(edges, EdgeType::Produces, |edge| {
            edge.from_type == NodeType::Requirement
                && edge.from_id == *id
                && edge.to_type == NodeType::Rule
        }));
        let mut desired = selected(&self.references, |relation| relation.3 == id.as_str());
        desired.extend(selected(&self.produces, |relation| {
            relation.1 == id.as_str()
        }));
        Self::add_change(current, desired, changes);
    }

    pub(super) fn add_rule_change(
        &self,
        id: &StableId,
        edges: &[Edge],
        changes: &mut Vec<TypedFieldChange>,
    ) {
        Self::add_change(
            current_relationships(edges, EdgeType::Produces, |edge| {
                edge.from_type == NodeType::Requirement
                    && edge.to_type == NodeType::Rule
                    && edge.to_id == *id
            }),
            selected(&self.produces, |relation| relation.3 == id.as_str()),
            changes,
        );
    }

    fn add_change(
        current: BTreeSet<Relationship>,
        desired: BTreeSet<Relationship>,
        changes: &mut Vec<TypedFieldChange>,
    ) {
        if current != desired {
            changes.push(TypedFieldChange {
                field: "relationships".to_string(),
                before: serde_json::to_value(current).expect("relationships serialize"),
                after: serde_json::to_value(desired).expect("relationships serialize"),
            });
        }
    }
}

fn current_relationships(
    edges: &[Edge],
    edge_type: EdgeType,
    relevant: impl Fn(&Edge) -> bool,
) -> BTreeSet<Relationship> {
    edges
        .iter()
        .filter(|edge| edge.edge_type == edge_type && relevant(edge))
        .map(|edge| relation(edge.from_type, &edge.from_id, edge.to_type, &edge.to_id))
        .collect()
}

fn selected(
    relationships: &BTreeSet<Relationship>,
    relevant: impl Fn(&Relationship) -> bool,
) -> BTreeSet<Relationship> {
    relationships
        .iter()
        .filter(|relationship| relevant(relationship))
        .cloned()
        .collect()
}

fn relation(
    from_type: NodeType,
    from_id: &StableId,
    to_type: NodeType,
    to_id: &StableId,
) -> Relationship {
    (
        relationship_node(from_type),
        from_id.as_str().to_string(),
        relationship_node(to_type),
        to_id.as_str().to_string(),
    )
}

const fn relationship_node(node_type: NodeType) -> RelationshipNode {
    match node_type {
        NodeType::Source => RelationshipNode::Source,
        NodeType::Requirement => RelationshipNode::Requirement,
        NodeType::Resolution => RelationshipNode::Resolution,
        NodeType::Rule => RelationshipNode::Rule,
        NodeType::Topic => RelationshipNode::Topic,
        NodeType::Question => RelationshipNode::Question,
    }
}
