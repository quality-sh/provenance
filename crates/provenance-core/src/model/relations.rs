//! The closed vocabulary of relations between canonical records.
//!
//! A relation is a reference field on the record that makes the claim, and
//! the field carries its declaration: target kind, flow, and whether the
//! record requires it. Every traversal, reverse lookup, gap, validator
//! check, and projection row derives from the declared tables, so a
//! reference field without a declaration cannot exist: the derive refuses
//! it at compile time.
//!
//! This is a closed parameterization of fixed operations. It carries no
//! predicate language and no composition grammar, and it must not grow one.

mod decl;
mod front;
mod integrity;

pub use decl::{declaration_of, RelationDecl, RelationFlow, RelationOwner};
pub use front::{
    declaration_for, flow_neighbors, incoming_of, link_rows_of, link_target, outgoing_of,
    related_nodes, rows_of, RecordFront, RelatedNode, RelationDirection, RelationEndpoint,
    RelationRow, RelationSource, LINKS,
};
pub use integrity::{
    cycle_in, cycle_refusal, kind_word, missing_required, reaches, required_refusal, RelationCycle,
};

use super::shaping::{Boundary, Question, Topic};
use super::{Requirement, Resolution, Rule, Source};
use provenance_macros::rule;

/// The declaration tables of the seven owner kinds, in node rank order.
///
/// Each table is written by `#[derive(Relations)]` from the record's own
/// fields; this list is the one hand-written concatenation.
#[rule("rule_prov_relation_vocabulary_closed")]
pub const fn declared_relations() -> &'static [&'static [RelationDecl]] {
    &[
        &Source::RELATIONS,
        &Requirement::RELATIONS,
        &Resolution::RELATIONS,
        &Rule::RELATIONS,
        &Topic::RELATIONS,
        &Question::RELATIONS,
        &Boundary::RELATIONS,
    ]
}

/// True when some declaration, or `links`, carries the name.
pub fn is_relation_name(name: &str) -> bool {
    name == LINKS
        || declared_relations()
            .iter()
            .flat_map(|table| table.iter())
            .any(|decl| decl.name == name)
}

/// The clause a retired edge name carries in a refusal: the field or
/// record that holds the fact now, or that the fact is gone.
fn retired_clause(name: &str) -> Option<&'static str> {
    match name {
        "references" => Some("it is now cites"),
        "refines_into" => Some("it is now refines"),
        "produces" => Some("it is now requirement_ids/resolution_ids on the rule"),
        "resolves" => Some("it is now requirement_ids on the resolution"),
        "spawns" => Some("it is now spawned_by on the requirement"),
        "superseded_by" => Some("it is now supersedes on the newer record"),
        "needs" => Some("it is removed"),
        _ => None,
    }
}

/// The valid relation names in alphabetical order.
pub fn relation_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = declared_relations()
        .iter()
        .flat_map(|table| table.iter())
        .map(|decl| decl.name)
        .collect();
    names.push(LINKS);
    names.sort_unstable();
    names.dedup();
    names
}

/// How a filter naming an unknown relation is refused: the replacement
/// for a retired name, then the closed vocabulary.
pub fn unknown_relation_refusal(name: &str) -> String {
    let retired = retired_clause(name)
        .map(|clause| format!("; {clause}"))
        .unwrap_or_default();
    format!(
        "unknown relation `{name}`{retired}. valid relations: {}",
        relation_names()
            .iter()
            .map(|relation| format!("`{relation}`"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
