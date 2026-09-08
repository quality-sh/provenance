//! Resources required before an operation can run.
use crate::operations::read_policy::ReadPolicy;
use camino::Utf8PathBuf;
use provenance_core::{
    protocol::{
        failure::OperationFailure,
        repository::{RepositoryContext, RepositoryTarget},
    },
    ScopeId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionNeed {
    GraphStorage,
    RepositoryFiles,
    Git,
    RunStorage,
    ProjectionMaintenance,
}
pub type ExecutionNeeds = &'static [ExecutionNeed];

/// A native caller or an authorized host supplies the resolved binding once.
#[derive(Debug, Clone)]
pub struct PreparedContext {
    read: Option<PreparedRead>,
    repository: Option<PreparedRepository>,
}
#[derive(Debug, Clone)]
pub struct PreparedRepository {
    pub root: Utf8PathBuf,
    pub requested_target: String,
}
#[derive(Debug, Clone)]
pub struct PreparedRead {
    pub root: Utf8PathBuf,
    pub scope: ScopeId,
    pub policy: ReadPolicy,
    pub requested_target: String,
    pub external: bool,
}
impl PreparedContext {
    pub const fn data_free() -> Self {
        Self {
            read: None,
            repository: None,
        }
    }
    pub fn read(read: PreparedRead) -> Self {
        Self {
            repository: Some(PreparedRepository {
                root: read.root.clone(),
                requested_target: read.requested_target.clone(),
            }),
            read: Some(read),
        }
    }
    pub const fn for_repository(repository: PreparedRepository) -> Self {
        Self {
            repository: Some(repository),
            read: None,
        }
    }
    pub(super) fn repository(self) -> Result<PreparedRepository, OperationFailure> {
        self.repository.ok_or(OperationFailure::UnavailableNeeds)
    }
    pub(super) fn graph(self) -> Result<PreparedRead, OperationFailure> {
        self.read.ok_or(OperationFailure::UnavailableNeeds)
    }
    pub(super) fn prepare(self, needs: ExecutionNeeds) -> Result<Self, OperationFailure> {
        if needs.iter().all(|need| match need {
            ExecutionNeed::GraphStorage => self.repository.is_some(),
            ExecutionNeed::ProjectionMaintenance => self.read.is_some(),
            _ => false,
        }) {
            Ok(self)
        } else {
            Err(OperationFailure::UnavailableNeeds)
        }
    }
}

/// Hosts authorize the selected target before reading settings or preparing storage.
pub trait ContextResolver: Send + Sync {
    fn prepare(
        &self,
        operation: &'static str,
        context: RequestedContext,
        needs: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure>;
}
pub(super) struct NoRepositories;
impl ContextResolver for NoRepositories {
    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Err(OperationFailure::UnavailableNeeds)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ContextKind {
    DataFree,
    Repository,
    Scoped,
}
#[derive(Debug, Clone)]
pub enum RequestedContext {
    Repository(RepositoryTarget),
    Scoped(RepositoryContext),
}
