//! The Requirement review records of a scope import.

use super::requirement_reviews::Restatement;
use super::StateStore;
use provenance_core::{Requirement, ScopeId};

/// The statements that a scope import changes, which the import puts up for review after it
/// writes the scope.
pub(super) struct ImportReviews {
    restatements: Vec<Restatement>,
}

impl ImportReviews {
    /// Reads the stored Requirements, before the import removes them, against the imported
    /// `requirements`.
    pub(super) fn read(
        store: &StateStore,
        scope: &ScopeId,
        requirements: &[Requirement],
    ) -> anyhow::Result<Self> {
        let restatements = store
            .list_requirements(scope)?
            .into_iter()
            .filter_map(|stored| {
                let imported = requirements.iter().find(|record| record.id == stored.id)?;
                (imported.statement != stored.statement).then(|| Restatement {
                    requirement_id: stored.id,
                    before: stored.statement,
                    after: imported.statement.clone(),
                })
            })
            .collect();
        Ok(Self { restatements })
    }

    /// Writes one review record for each Rule that cites a restated Requirement in the scope
    /// that the import wrote.
    pub(super) fn write(self, store: &StateStore, scope: &ScopeId) -> anyhow::Result<()> {
        store.record_restatements(scope, self.restatements)
    }
}
