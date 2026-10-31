//! The graph validator: what a scope's relation fields must hold before
//! the scope is published, materialized, or exported.
//!
//! The ideation validator reads only the ideation families, so it cannot
//! see a rule, a resolution, or a requirement. This one reads the seven
//! kinds that declare relations and refuses two things serde lets through:
//! an empty required list, and a chain over a relation that targets its
//! own kind leading back to its start.
//!
//! The chain set is derived, not hand-listed: every relation whose target
//! kind equals its owner kind joins the cycle check, so a future
//! self-referencing relation is covered without a new list.

use super::StateStore;
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::model::relations::{
    cycle_in, cycle_refusal, kind_word, missing_required, required_refusal, RecordFront,
    RelationOwner,
};
use provenance_core::ScopeId;

impl StateStore {
    /// Refuses a scope whose relation fields do not hold. Runs under the
    /// lock the caller already holds.
    pub fn validate_graph_scope(&self, scope: &ScopeId) -> anyhow::Result<()> {
        let sources = self.list_sources(scope)?;
        let requirements = self.list_requirements(scope)?;
        let resolutions = self.list_resolutions(scope)?;
        let rules = self.list_rules(scope)?;
        let topics = self.list_topics(scope)?;
        let questions = self.list_questions(scope)?;
        let domains = self.list_domains(scope)?;
        let boundaries = self.list_boundaries(scope)?;
        validate_relations(&RecordFront {
            sources: &sources,
            requirements: &requirements,
            resolutions: &resolutions,
            rules: &rules,
            topics: &topics,
            questions: &questions,
            domains: &domains,
            boundaries: &boundaries,
        })
    }
}

/// Refuses records whose relation fields do not hold: an archive stamp
/// that does not match the status, an empty required list, or a chain
/// that returns to its start. Reads only the front; domains carry no
/// relation this checks.
pub fn validate_relations(front: &RecordFront<'_>) -> anyhow::Result<()> {
    ensure_archives(
        front.requirements,
        provenance_core::Requirement::validate_archive,
    )?;
    ensure_archives(
        front.resolutions,
        provenance_core::Resolution::validate_archive,
    )?;
    ensure_archives(front.rules, provenance_core::Rule::validate_archive)?;
    ensure_archives(front.topics, provenance_core::Topic::validate_archive)?;
    ensure_archives(front.questions, provenance_core::Question::validate_archive)?;
    ensure_required(front.sources)?;
    ensure_required(front.requirements)?;
    ensure_required(front.resolutions)?;
    ensure_required(front.rules)?;
    ensure_required(front.topics)?;
    ensure_required(front.questions)?;
    ensure_required(front.boundaries)?;
    ensure_acyclic(front.requirements)?;
    ensure_acyclic(front.sources)?;
    ensure_acyclic(front.resolutions)?;
    ensure_acyclic(front.rules)?;
    ensure_acyclic(front.topics)?;
    ensure_acyclic(front.questions)?;
    ensure_acyclic(front.boundaries)
}

fn ensure_archives<T>(
    records: &[T],
    validate: impl Fn(&T) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    for record in records {
        validate(record)?;
    }
    Ok(())
}

fn ensure_required<T: RelationOwner>(records: &[T]) -> anyhow::Result<()> {
    for record in records {
        if let Some(decl) = missing_required(record) {
            return Err(SourceFailure::wrap(
                WriteFailure::InvalidUpdate,
                anyhow::anyhow!(
                    "{} {} is refused: {}",
                    kind_word(T::OWNER),
                    record.id().as_str(),
                    required_refusal(decl)
                ),
            ));
        }
    }
    Ok(())
}

/// Refuses every chain over a relation the kind carries to its own kind.
fn ensure_acyclic<T: RelationOwner>(records: &[T]) -> anyhow::Result<()> {
    for name in chain_names::<T>() {
        if let Some(cycle) = cycle_in(records, name) {
            return Err(SourceFailure::wrap(
                WriteFailure::InvalidUpdate,
                anyhow::anyhow!("{}", cycle_refusal(name, &cycle)),
            ));
        }
    }
    Ok(())
}

/// The relation names this kind carries to its own kind, read from the
/// declaration table the derive wrote.
fn chain_names<T: RelationOwner>() -> Vec<&'static str> {
    T::relations()
        .iter()
        .filter(|decl| decl.target == T::OWNER)
        .map(|decl| decl.name)
        .collect()
}
