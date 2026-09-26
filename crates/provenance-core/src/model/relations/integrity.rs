//! The two checks every writer, validator, and merge gate run over the
//! declared relations: a required relation is never empty, and a relation
//! a record kind carries to its own kind never loops.

use super::decl::{RelationDecl, RelationOwner};
use crate::model::graph::NodeType;
use crate::model::ids::StableId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
    incoming: BTreeMap<String, Vec<StableId>>,
}

impl RelationIndex {
    fn new<T: RelationOwner>(records: &[T], name: &str) -> Self {
        let mut incoming = BTreeMap::<String, Vec<StableId>>::new();
        for record in records {
            for (_, target) in record
                .references()
                .into_iter()
                .filter(|(relation, _)| *relation == name)
            {
                incoming
                    .entry(target.as_str().to_owned())
                    .or_default()
                    .push(record.id().clone());
            }
        }
        for owners in incoming.values_mut() {
            owners.sort_by(|left, right| left.as_str().cmp(right.as_str()));
            owners.dedup();
        }
        Self { incoming }
    }

    fn paths_to(&self, wanted: &StableId) -> PathsToTarget {
        let mut next_hop = BTreeMap::new();
        let mut seen = BTreeSet::from([wanted.as_str().to_owned()]);
        let mut pending = VecDeque::from([wanted.clone()]);
        while let Some(current) = pending.pop_front() {
            if let Some(owners) = self.incoming.get(current.as_str()) {
                for owner in owners {
                    if seen.insert(owner.as_str().to_owned()) {
                        next_hop.insert(owner.as_str().to_owned(), current.clone());
                        pending.push_back(owner.clone());
                    }
                }
            }
        }
        PathsToTarget {
            wanted: wanted.clone(),
            next_hop,
        }
    }
}

struct PathsToTarget {
    wanted: StableId,
    next_hop: BTreeMap<String, StableId>,
}

impl PathsToTarget {
    fn contains(&self, start: &StableId) -> bool {
        start == &self.wanted || self.next_hop.contains_key(start.as_str())
    }

    fn path_from(&self, start: &StableId) -> Option<Vec<StableId>> {
        if !self.contains(start) {
            return None;
        }
        let mut path = vec![start.clone()];
        while path.last() != Some(&self.wanted) {
            let next = self
                .next_hop
                .get(path.last().expect("a path has a current node").as_str())
                .expect("each reachable node has a next hop");
            path.push(next.clone());
        }
        Some(path)
    }
}

/// The first cycle of this relation in record order, with the pair that
/// closes it and the path around it.
pub fn cycle_in<T: RelationOwner>(records: &[T], name: &str) -> Option<RelationCycle> {
    let index = RelationIndex::new(records, name);
    for record in records {
        let paths = index.paths_to(record.id());
        for (relation, target) in record.references() {
            if relation != name {
                continue;
            }
            if let Some(walked) = paths.path_from(target) {
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
    let paths = index.paths_to(owner);
    for target in targets {
        if let Some(mut path) = paths.path_from(target) {
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
    RelationIndex::new(records, name)
        .paths_to(wanted)
        .contains(start)
}
