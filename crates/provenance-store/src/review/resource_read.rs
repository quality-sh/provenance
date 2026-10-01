//! One coherent read of a graph record and its mutable review state.

use crate::state_store::StateStore;
use provenance_core::review::{RequirementDecisionState, RequirementEditState};
use provenance_core::{review::ReviewRecord, NodeType, Requirement, ScopeId, StableId};

pub struct RecordResourceSnapshot {
    pub record: ReviewRecord,
    pub edit: RequirementEditState,
    pub decision: RequirementDecisionState,
}

pub struct RequirementResourceSnapshot {
    pub record: Requirement,
    pub edit: RequirementEditState,
    pub decision: RequirementDecisionState,
}

impl StateStore {
    /// Reads one reviewable record and its review state under one lock.
    pub(crate) fn record_resource_snapshot(
        &self,
        scope: &ScopeId,
        kind: NodeType,
        id: &StableId,
    ) -> anyhow::Result<RecordResourceSnapshot> {
        self.with_repository_publication(|| self.record_resource_snapshot_unlocked(scope, kind, id))
    }

    pub(super) fn requirement_resource_snapshot_unlocked(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<RequirementResourceSnapshot> {
        let snapshot = self.record_resource_snapshot_unlocked(scope, NodeType::Requirement, id)?;
        let record = snapshot
            .record
            .as_requirement()
            .expect("Requirement lookup returns a Requirement")
            .clone();
        Ok(RequirementResourceSnapshot {
            record,
            edit: snapshot.edit,
            decision: snapshot.decision,
        })
    }

    fn record_resource_snapshot_unlocked(
        &self,
        scope: &ScopeId,
        kind: NodeType,
        id: &StableId,
    ) -> anyhow::Result<RecordResourceSnapshot> {
        crate::test_probes::at("requirement_resource_snapshot")?;
        let record = crate::cache::review_families::record(self, scope, kind, id)?;
        #[cfg(feature = "test-fixture")]
        if kind == NodeType::Requirement {
            crate::fixture_probe::at("requirement_resource_record_read");
        }
        Ok(RecordResourceSnapshot {
            edit: self.record_edit_state_for_record(&record)?,
            decision: self.record_decision_state_for_record(&record)?,
            record,
        })
    }
}
