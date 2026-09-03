use std::collections::BTreeSet;

use provenance_core::{NodeType, Requirement, Rule, StableId};

use super::super::{rule_address, CurrentTypedState, DesiredTypedIds};
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
    Domain,
    Boundary,
}

/// One relation as the adoption report spells it: the source end first,
/// the requirement or rule it reaches second.
type Relationship = (RelationshipNode, String, RelationshipNode, String);

/// The citations and rule lists a declaration asks for, beside the ones the
/// records hold today. A declaration is exact only when both agree.
pub(super) struct DesiredRelationships {
    references: BTreeSet<Relationship>,
    produces: BTreeSet<Relationship>,
}

/// The records the comparison reads: a requirement's `cites` and a rule's
/// `requirement_ids`.
#[derive(Clone, Copy)]
pub(super) struct CurrentRelationships<'a> {
    pub(super) requirements: &'a [Requirement],
    pub(super) rules: &'a [Rule],
}

impl CurrentTypedState {
    /// The records the relationship comparison reads.
    pub(super) fn relationships(&self) -> CurrentRelationships<'_> {
        CurrentRelationships {
            requirements: &self.requirements,
            rules: &self.rules,
        }
    }
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

    pub(super) fn source_matches(&self, id: &StableId, current: CurrentRelationships<'_>) -> bool {
        cited_by(current, |_, source| source == id)
            == selected(&self.references, |relation| relation.1 == id.as_str())
    }

    pub(super) fn requirement_matches(
        &self,
        id: &StableId,
        current: CurrentRelationships<'_>,
    ) -> bool {
        Self::requirement_current(id, current) == self.requirement_desired(id)
    }

    pub(super) fn rule_matches(&self, id: &StableId, current: CurrentRelationships<'_>) -> bool {
        produced(current, |_, rule| rule == id)
            == selected(&self.produces, |relation| relation.3 == id.as_str())
    }

    pub(super) fn add_source_change(
        &self,
        id: &StableId,
        current: CurrentRelationships<'_>,
        changes: &mut Vec<TypedFieldChange>,
    ) {
        Self::add_change(
            cited_by(current, |_, source| source == id),
            selected(&self.references, |relation| relation.1 == id.as_str()),
            changes,
        );
    }

    pub(super) fn add_requirement_change(
        &self,
        id: &StableId,
        current: CurrentRelationships<'_>,
        changes: &mut Vec<TypedFieldChange>,
    ) {
        Self::add_change(
            Self::requirement_current(id, current),
            self.requirement_desired(id),
            changes,
        );
    }

    pub(super) fn add_rule_change(
        &self,
        id: &StableId,
        current: CurrentRelationships<'_>,
        changes: &mut Vec<TypedFieldChange>,
    ) {
        Self::add_change(
            produced(current, |_, rule| rule == id),
            selected(&self.produces, |relation| relation.3 == id.as_str()),
            changes,
        );
    }

    fn requirement_current(
        id: &StableId,
        current: CurrentRelationships<'_>,
    ) -> BTreeSet<Relationship> {
        let mut rows = cited_by(current, |requirement, _| requirement == id);
        rows.extend(produced(current, |requirement, _| requirement == id));
        rows
    }

    fn requirement_desired(&self, id: &StableId) -> BTreeSet<Relationship> {
        let mut rows = selected(&self.references, |relation| relation.3 == id.as_str());
        rows.extend(selected(&self.produces, |relation| {
            relation.1 == id.as_str()
        }));
        rows
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

/// The citations held today, as (source, requirement) rows the filter admits.
fn cited_by(
    current: CurrentRelationships<'_>,
    relevant: impl Fn(&StableId, &StableId) -> bool,
) -> BTreeSet<Relationship> {
    current
        .requirements
        .iter()
        .flat_map(|requirement| {
            requirement
                .source_refs
                .iter()
                .filter(|reference| relevant(&requirement.id, &reference.source_id))
                .map(|reference| {
                    relation(
                        NodeType::Source,
                        &reference.source_id,
                        NodeType::Requirement,
                        &requirement.id,
                    )
                })
        })
        .collect()
}

/// The rule lists held today, as (requirement, rule) rows the filter admits.
fn produced(
    current: CurrentRelationships<'_>,
    relevant: impl Fn(&StableId, &StableId) -> bool,
) -> BTreeSet<Relationship> {
    current
        .rules
        .iter()
        .flat_map(|rule| {
            rule.requirement_ids
                .iter()
                .filter(|requirement| relevant(requirement, &rule.id))
                .map(|requirement| {
                    relation(NodeType::Requirement, requirement, NodeType::Rule, &rule.id)
                })
        })
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
        NodeType::Domain => RelationshipNode::Domain,
        NodeType::Boundary => RelationshipNode::Boundary,
    }
}
