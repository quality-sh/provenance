//! The graph validator: what a scope's relation fields must hold before
//! the scope is published, materialized, or exported.
//!
//! The ideation validator reads only the ideation families, so it cannot
//! see a rule, a resolution, or a requirement. This one reads the seven
//! kinds that declare relations and refuses two things serde lets through:
//! an empty required list, and a `refines`, `depends_on`, or `supersedes`
//! chain that leads back to its own record.

use super::StateStore;
use provenance_core::model::relations::{
    cycle_in, cycle_refusal, kind_word, missing_required, required_refusal, RelationOwner,
};
use provenance_core::{Requirement, ScopeId};

/// The requirement relations a record kind carries to its own kind.
const REQUIREMENT_CHAINS: [&str; 3] = ["refines", "depends_on", "supersedes"];

impl StateStore {
    /// Refuses a scope whose relation fields do not hold. Runs under the
    /// lock the caller already holds.
    pub fn validate_graph_scope(&self, scope: &ScopeId) -> anyhow::Result<()> {
        ensure_required(&self.list_sources(scope)?)?;
        let requirements = self.list_requirements(scope)?;
        ensure_required(&requirements)?;
        ensure_required(&self.list_resolutions(scope)?)?;
        ensure_required(&self.list_rules(scope)?)?;
        ensure_required(&self.list_topics(scope)?)?;
        ensure_required(&self.list_questions(scope)?)?;
        ensure_required(&self.list_boundaries(scope)?)?;
        ensure_acyclic(&requirements)
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

fn ensure_acyclic(requirements: &[Requirement]) -> anyhow::Result<()> {
    for name in REQUIREMENT_CHAINS {
        if let Some((from, through)) = cycle_in(requirements, name) {
            anyhow::bail!("{}", cycle_refusal(name, &from, &through));
        }
    }
    Ok(())
}
