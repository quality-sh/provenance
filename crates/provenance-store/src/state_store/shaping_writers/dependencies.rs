//! Validate Question prerequisites before the record is published.
use crate::state_store::StateStore;
use crate::write_error::{SourceFailure, WriteFailure};
use provenance_core::{ScopeId, StableId};
use std::collections::{BTreeMap, BTreeSet};

impl StateStore {
    pub(crate) fn checked_question_dependencies(
        &self,
        scope: &ScopeId,
        id: &StableId,
        topic: &StableId,
        mut dependencies: Vec<StableId>,
    ) -> anyhow::Result<Vec<StableId>> {
        dependencies.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        dependencies.dedup();
        let questions = self.list_questions(scope)?;
        let by_id: BTreeMap<_, _> = questions
            .iter()
            .map(|question| (question.id.as_str(), question))
            .collect();
        for dependency in &dependencies {
            crate::write_error::ensure!(
                InvalidUpdate,
                dependency != id,
                "a Question cannot depend on itself"
            );
            let question = by_id.get(dependency.as_str()).ok_or_else(|| {
                SourceFailure::wrap(
                    WriteFailure::MissingReference,
                    anyhow::anyhow!(
                        "prerequisite Question does not exist: {}",
                        dependency.as_str()
                    ),
                )
            })?;
            crate::write_error::ensure!(
                InvalidUpdate,
                &question.topic_id == topic,
                "prerequisite Questions must belong to the same Topic"
            );
        }
        let mut pending: Vec<_> = dependencies.iter().map(StableId::as_str).collect();
        let mut seen = BTreeSet::new();
        while let Some(next) = pending.pop() {
            crate::write_error::ensure!(
                InvalidUpdate,
                next != id.as_str(),
                "Question prerequisites contain a cycle"
            );
            if seen.insert(next) {
                if let Some(question) = by_id.get(next) {
                    pending.extend(question.depends_on.iter().map(StableId::as_str));
                }
            }
        }
        Ok(dependencies)
    }
}
