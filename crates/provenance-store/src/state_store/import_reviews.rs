//! The Requirement review records of a scope import.

use super::requirement_reviews::Restatement;
use super::StateStore;
use crate::shards;
use provenance_core::{Requirement, RequirementReview, Rule, ScopeId, StableId};

/// The review records that a scope import writes after it writes the scope: each record that
/// the scope held, and one record for each Rule of each Requirement whose statement the import
/// changes.
pub(super) struct ImportReviews {
    stored: Vec<RequirementReview>,
    restatements: Vec<Restatement>,
}

impl ImportReviews {
    /// Reads the stored scope, before the import removes it, against the imported
    /// `requirements` and `rules`.
    pub(super) fn read(
        store: &StateStore,
        scope: &ScopeId,
        requirements: &[Requirement],
        rules: &[Rule],
    ) -> anyhow::Result<Self> {
        let stored_rules = store.list_rules(scope)?;
        let restatements = store
            .list_requirements(scope)?
            .into_iter()
            .filter_map(|stored| {
                let imported = requirements.iter().find(|record| record.id == stored.id)?;
                (imported.statement != stored.statement).then(|| Restatement {
                    rule_ids: rules_citing(&stored.id, &stored_rules, rules),
                    requirement_id: stored.id,
                    before: stored.statement,
                    after: imported.statement.clone(),
                })
            })
            .collect();
        Ok(Self {
            stored: store.list_requirement_reviews(scope)?,
            restatements,
        })
    }

    /// Writes the records into the scope that the import wrote.
    pub(super) fn write(self, store: &StateStore, scope: &ScopeId) -> anyhow::Result<()> {
        if !self.stored.is_empty() {
            let path = shards::requirement_reviews_path(&store.layout, scope);
            store.mutate_jsonl_records(&path, |records: &mut Vec<RequirementReview>| {
                *records = self.stored;
                Ok(())
            })?;
        }
        store.record_restatements(scope, self.restatements)
    }
}

/// The Rules that cite `requirement` before or after the import, the stored Rules first.
fn rules_citing(requirement: &StableId, stored: &[Rule], imported: &[Rule]) -> Vec<StableId> {
    let mut ids: Vec<StableId> = Vec::new();
    for rule in stored.iter().chain(imported) {
        if rule.requirement_ids.contains(requirement) && !ids.contains(&rule.id) {
            ids.push(rule.id.clone());
        }
    }
    ids
}
