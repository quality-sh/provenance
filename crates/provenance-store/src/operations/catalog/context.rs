//! Resources required before an operation can run.

use provenance_core::protocol::failure::OperationFailure;

/// Resource use, independent of the caller's access grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionNeed {
    GraphStorage,
    RepositoryFiles,
    Git,
    RunStorage,
    ProjectionMaintenance,
}

pub type ExecutionNeeds = &'static [ExecutionNeed];

/// A prepared data-free call cannot carry a repository or load its settings.
#[derive(Debug, Clone, Copy)]
pub struct PreparedContext {
    _private: (),
}

impl PreparedContext {
    pub const fn data_free() -> Self {
        Self { _private: () }
    }

    pub(super) const fn prepare(self, needs: ExecutionNeeds) -> Result<Self, OperationFailure> {
        if needs.is_empty() {
            Ok(self)
        } else {
            Err(OperationFailure::UnavailableNeeds)
        }
    }
}
