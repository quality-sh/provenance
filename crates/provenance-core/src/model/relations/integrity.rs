//! The two checks every writer, validator, and merge gate run over the
//! declared relations: a required relation is never empty, and a relation
//! a record kind carries to its own kind never loops.

use super::decl::{RelationDecl, RelationOwner};
use crate::model::graph::NodeType;
use crate::model::ids::StableId;
use std::collections::BTreeMap;

/// The product word for one record kind.
pub const fn kind_word(node_type: NodeType) -> &'static str {
    match node_type {
        NodeType::Source => "source",
        NodeType::Requirement => "requirement",
        NodeType::Resolution => "resolution",
        NodeType::Rule => "rule",
        NodeType::Topic => "topic",
        NodeType::Question => "question",
        NodeType::Domain => "domain",
        NodeType::Boundary => "boundary",
    }
}

/// The first required relation this record leaves empty.
pub fn missing_required<T: RelationOwner>(record: &T) -> Option<&'static RelationDecl> {
    let references = record.references();
    T::relations()
        .iter()
        .find(|decl| decl.required && !references.iter().any(|(name, _)| *name == decl.name))
}

/// How an empty required relation is refused.
pub fn required_refusal(decl: &RelationDecl) -> String {
    format!(
        "a {} needs one {}",
        kind_word(decl.owner),
        kind_word(decl.target)
    )
}

/// A relation cycle in state: the pair whose stored reference closes it,
/// and the cycle read from its start back to that start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationCycle {
    /// The record whose stored reference re-enters the cycle's start.
    pub closes_from: StableId,
    /// The start the closing reference names.
    pub closes_into: StableId,
    /// The ids from the start through the closing reference back to the
    /// start; the first and the last are the same record.
    pub path: Vec<StableId>,
}

struct RelationIndex {
    outgoing: BTreeMap<String, Vec<StableId>>,
}

impl RelationIndex {
    fn new<T: RelationOwner>(records: &[T], name: &str) -> Self {
        let outgoing = records
            .iter()
            .map(|record| {
                let targets = record
                    .references()
                    .into_iter()
                    .filter(|(relation, _)| *relation == name)
                    .map(|(_, id)| id.clone())
                    .collect();
                (record.id().as_str().to_owned(), targets)
            })
            .collect();
        Self { outgoing }
    }

    fn path(&self, start: &StableId, wanted: &StableId) -> Option<Vec<StableId>> {
        let mut stack = vec![(start.clone(), vec![start.clone()])];
        let mut seen = Vec::new();
        while let Some((current, path)) = stack.pop() {
            if current == *wanted {
                return Some(path);
            }
            if seen.contains(&current) {
                continue;
            }
            seen.push(current.clone());
            if let Some(targets) = self.outgoing.get(current.as_str()) {
                for id in targets {
                    let mut next = path.clone();
                    next.push(id.clone());
                    stack.push((id.clone(), next));
                }
            }
        }
        None
    }
}

/// The first cycle of this relation in record order, with the pair that
/// closes it and the path around it.
pub fn cycle_in<T: RelationOwner>(records: &[T], name: &str) -> Option<RelationCycle> {
    let index = RelationIndex::new(records, name);
    for record in records {
        for (relation, target) in record.references() {
            if relation != name {
                continue;
            }
            if let Some(walked) = index.path(target, record.id()) {
                let mut path = vec![record.id().clone()];
                path.extend(walked);
                let closes_from = path[path.len() - 2].clone();
                return Some(RelationCycle {
                    closes_from,
                    closes_into: record.id().clone(),
                    path,
                });
            }
        }
    }
    None
}

/// The first cycle that one of the hypothetical owner-to-target edges would form.
pub fn cycle_with_added_edges<T: RelationOwner>(
    records: &[T],
    name: &str,
    owner: &StableId,
    targets: &[StableId],
) -> Option<RelationCycle> {
    let index = RelationIndex::new(records, name);
    for target in targets {
        if let Some(mut path) = index.path(target, owner) {
            path.push(target.clone());
            return Some(RelationCycle {
                closes_from: owner.clone(),
                closes_into: target.clone(),
                path,
            });
        }
    }
    None
}

/// How a cycle in state is refused: the relation and the cycle from its
/// start back to its start.
pub fn cycle_refusal(name: &str, cycle: &RelationCycle) -> String {
    let ids: Vec<&str> = cycle.path.iter().map(StableId::as_str).collect();
    format!("{name} forms a cycle: {}", ids.join(" -> "))
}

/// Whether following the named relation from `start` reaches `wanted`.
pub fn reaches<T: RelationOwner>(
    records: &[T],
    name: &str,
    start: &StableId,
    wanted: &StableId,
) -> bool {
    RelationIndex::new(records, name).path(start, wanted).is_some()
}
