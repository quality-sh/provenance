//! The two checks every writer, validator, and merge gate run over the
//! declared relations: a required relation is never empty, and a relation
//! a record kind carries to its own kind never loops.

use super::decl::{RelationDecl, RelationOwner};
use crate::model::graph::NodeType;
use crate::model::ids::StableId;

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

/// The first record whose relation of this name leads back to itself,
/// with the record it reaches it through.
pub fn cycle_in<T: RelationOwner>(records: &[T], name: &str) -> Option<(StableId, StableId)> {
    for record in records {
        for (relation, target) in record.references() {
            if relation == name && reaches(records, name, target, record.id()) {
                return Some((record.id().clone(), target.clone()));
            }
        }
    }
    None
}

/// Whether following the named relation from `start` reaches `wanted`.
pub fn reaches<T: RelationOwner>(
    records: &[T],
    name: &str,
    start: &StableId,
    wanted: &StableId,
) -> bool {
    let mut stack = vec![start.clone()];
    let mut seen = Vec::new();
    while let Some(current) = stack.pop() {
        if current == *wanted {
            return true;
        }
        if seen.contains(&current) {
            continue;
        }
        seen.push(current.clone());
        if let Some(record) = records.iter().find(|record| *record.id() == current) {
            stack.extend(
                record
                    .references()
                    .into_iter()
                    .filter(|(relation, _)| *relation == name)
                    .map(|(_, id)| id.clone()),
            );
        }
    }
    false
}

/// How a cycle is refused.
pub fn cycle_refusal(name: &str, from: &StableId, through: &StableId) -> String {
    format!(
        "{name} from {} to {} would form a cycle",
        from.as_str(),
        through.as_str()
    )
}
