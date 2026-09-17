//! One coherent read of a Requirement and its mutable review state.

use crate::state_store::StateStore;
use provenance_core::review::{RequirementDecisionState, RequirementEditState};
use provenance_core::{Requirement, ScopeId, StableId};

pub struct RequirementResourceSnapshot {
    pub record: Requirement,
    pub edit: RequirementEditState,
    pub decision: RequirementDecisionState,
}

impl StateStore {
    /// Reads the complete resource while one publication lock excludes saves.
    pub(crate) fn requirement_resource_snapshot(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<RequirementResourceSnapshot> {
        self.with_repository_publication(|| {
            let record = self
                .list_requirements(scope)?
                .into_iter()
                .find(|record| record.id == *id)
                .ok_or(provenance_core::protocol::read_failure::ReadFailure::ResourceNotFound)?;
            #[cfg(feature = "test-fixture")]
            crate::fixture_probe::at("requirement_resource_record_read");
            Ok(RequirementResourceSnapshot {
                record,
                edit: self.requirement_edit_state(scope, id)?,
                decision: self.requirement_decision_state(scope, id)?,
            })
        })
    }
}
