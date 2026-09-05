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
use provenance_core::model::relations::{
    cycle_in, cycle_refusal, kind_word, missing_required, required_refusal, RelationOwner,
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
        let boundaries = self.list_boundaries(scope)?;
        ensure_required(&sources)?;
        ensure_required(&requirements)?;
        ensure_required(&resolutions)?;
        ensure_required(&rules)?;
        ensure_required(&topics)?;
        ensure_required(&questions)?;
        ensure_required(&boundaries)?;
        ensure_acyclic(&requirements)?;
        ensure_acyclic(&sources)?;
        ensure_acyclic(&resolutions)?;
        ensure_acyclic(&rules)?;
        ensure_acyclic(&topics)?;
        ensure_acyclic(&questions)?;
        ensure_acyclic(&boundaries)
    }
}

fn ensure_required<T: RelationOwner>(records: &[T]) -> anyhow::Result<()> {
    for record in records {
        if let Some(decl) = missing_required(record) {
            anyhow::bail!(
                "{} {} is refused: {}",
                kind_word(T::OWNER),
                record.id().as_str(),
                required_refusal(decl)
            );
        }
    }
    Ok(())
}

/// Refuses every chain over a relation the kind carries to its own kind.
fn ensure_acyclic<T: RelationOwner>(records: &[T]) -> anyhow::Result<()> {
    for name in chain_names::<T>() {
        if let Some(cycle) = cycle_in(records, name) {
            anyhow::bail!("{}", cycle_refusal(name, &cycle));
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
