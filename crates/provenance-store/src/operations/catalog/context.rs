//! Resources required before an operation can run.
use crate::operations::read_policy::ReadPolicy;
use crate::repository::Repository;
use provenance_core::protocol::{
    failure::OperationFailure,
    repository::{RepositoryContext, RepositoryScope, RepositoryTarget},
};
use provenance_macros::{rule, verifies};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionNeed {
    GraphStorage,
    Dictionary,
    RepositoryFiles,
    Git,
    RunStorage,
    ProjectionMaintenance,
}
pub type ExecutionNeeds = &'static [ExecutionNeed];

/// A native caller or an authorized host supplies the resolved binding once.
///
/// A catalog body receives only this context. It carries `Repository`
/// handles and no path, lock or connection, so a body reaches state only
/// through the handle and its stores.
#[derive(Debug, Clone)]
#[rule("rule_catalog_operations_name_no_storage_mechanics")]
#[verifies("rule_catalog_operations_name_no_storage_mechanics", construction)]
pub struct PreparedContext {
    read: Option<PreparedRead>,
    scope: Option<PreparedScope>,
    repository: Option<PreparedRepository>,
}
#[derive(Debug, Clone)]
pub struct PreparedRepository {
    pub repository: Repository,
    pub requested_target: String,
}
#[derive(Debug, Clone)]
pub struct PreparedScope {
    pub repository: Repository,
    pub requested_target: String,
}
#[derive(Debug, Clone)]
pub struct PreparedRead {
    pub repository: Repository,
    pub policy: ReadPolicy,
    pub requested_target: String,
    pub external: bool,
}
impl PreparedContext {
    pub const fn data_free() -> Self {
        Self {
            read: None,
            scope: None,
            repository: None,
        }
    }
    pub fn read(read: PreparedRead) -> Self {
        Self {
            repository: Some(PreparedRepository {
                repository: read.repository.clone(),
                requested_target: read.requested_target.clone(),
            }),
            scope: Some(PreparedScope {
                repository: read.repository.clone(),
                requested_target: read.requested_target.clone(),
            }),
            read: Some(read),
        }
    }
    pub const fn for_repository(repository: PreparedRepository) -> Self {
        Self {
            repository: Some(repository),
            read: None,
            scope: None,
        }
    }
    pub fn for_scope(scope: PreparedScope) -> Self {
        Self {
            repository: Some(PreparedRepository {
                repository: scope.repository.clone(),
                requested_target: scope.requested_target.clone(),
            }),
            read: None,
            scope: Some(scope),
        }
    }
    /// Use native answer framing after a host has authorized this context.
    #[must_use]
    pub const fn into_native(mut self) -> Self {
        if let Some(read) = &mut self.read {
            read.external = false;
        }
        self
    }
    /// Binds the prepared repository to `held`, the repository of the section
    /// that the operation runs in. `held` must have the same working tree and
    /// scope; another repository is refused with
    /// [`OperationFailure::Internal`].
    pub fn within(mut self, held: &Repository) -> Result<Self, OperationFailure> {
        let rebind = |prepared: &mut Repository| {
            if prepared.working_tree() == held.working_tree() && prepared.scope() == held.scope() {
                *prepared = held.clone();
                Ok(())
            } else {
                Err(OperationFailure::Internal)
            }
        };
        if let Some(read) = &mut self.read {
            rebind(&mut read.repository)?;
        }
        if let Some(scope) = &mut self.scope {
            rebind(&mut scope.repository)?;
        }
        if let Some(repository) = &mut self.repository {
            rebind(&mut repository.repository)?;
        }
        Ok(self)
    }
    pub(super) fn scope(self) -> Result<PreparedScope, OperationFailure> {
        self.scope.ok_or(OperationFailure::UnavailableNeeds)
    }
    pub(super) fn repository(self) -> Result<PreparedRepository, OperationFailure> {
        self.repository.ok_or(OperationFailure::UnavailableNeeds)
    }
    pub(super) fn graph(self) -> Result<PreparedRead, OperationFailure> {
        self.read.ok_or(OperationFailure::UnavailableNeeds)
    }
    pub(super) const fn validate_kind(&self, kind: ContextKind) -> Result<(), OperationFailure> {
        let valid = match kind {
            ContextKind::DataFree => {
                self.repository.is_none() && self.scope.is_none() && self.read.is_none()
            }
            ContextKind::Repository => self.repository.is_some(),
            ContextKind::Scope => self.scope.is_some(),
            ContextKind::Scoped => self.read.is_some(),
        };
        if valid {
            Ok(())
        } else {
            Err(OperationFailure::UnavailableNeeds)
        }
    }
    pub(super) fn prepare(self, needs: ExecutionNeeds) -> Result<Self, OperationFailure> {
        if needs.iter().all(|need| match need {
            ExecutionNeed::Dictionary
            | ExecutionNeed::GraphStorage
            | ExecutionNeed::Git
            | ExecutionNeed::RunStorage => self.repository.is_some(),
            ExecutionNeed::ProjectionMaintenance => self.read.is_some(),
            ExecutionNeed::RepositoryFiles => self.repository.is_some() && cfg!(any(unix, windows)),
        }) {
            Ok(self)
        } else {
            Err(OperationFailure::UnavailableNeeds)
        }
    }
}

/// Hosts authorize the selected target before reading settings or preparing storage.
pub trait ContextResolver: Send + Sync {
    /// Identity only, no repository IO. Runs on the runtime thread before the work slot wait.
    fn authorize(
        &self,
        operation: &'static str,
        context: &RequestedContext,
    ) -> Result<(), OperationFailure>;
    /// Repository IO: manifest grant, settings, read policy. Runs on the blocking path and
    /// repeats the identity check.
    fn prepare(
        &self,
        operation: &'static str,
        context: RequestedContext,
        needs: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure>;
}
/// Tests only: a resolver with no repositories.
#[cfg(test)]
pub(super) struct NoRepositories;
#[cfg(test)]
impl ContextResolver for NoRepositories {
    fn authorize(&self, _: &'static str, _: &RequestedContext) -> Result<(), OperationFailure> {
        Err(OperationFailure::UnavailableNeeds)
    }

    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Err(OperationFailure::UnavailableNeeds)
    }
}

#[cfg(test)]
mod tests {
    use super::{PreparedContext, PreparedRepository};
    use provenance_core::protocol::failure::OperationFailure;

    fn context(repository: &crate::repository::Repository) -> PreparedContext {
        PreparedContext::for_repository(PreparedRepository {
            repository: repository.clone(),
            requested_target: "repo".to_owned(),
        })
    }

    #[test]
    fn within_binds_the_same_repository_and_refuses_another() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let prepared = crate::cache::tests::fixtures::initialized(first.path());
        let same = crate::repository::Repository::open(prepared.working_tree(), "default").unwrap();
        let other = crate::cache::tests::fixtures::initialized(second.path());
        let mut manifest = prepared.manifest().unwrap();
        let mut extra = manifest.scopes[0].clone();
        extra.id = provenance_core::ScopeId::new("extra").unwrap();
        extra.path_prefix = provenance_core::RepoPathPrefix::new("extra");
        manifest.scopes.push(extra);
        std::fs::write(
            prepared.layout().manifest_path(),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();
        let other_scope =
            crate::repository::Repository::open(prepared.working_tree(), "extra").unwrap();

        let bound = context(&prepared).within(&same).unwrap();
        assert_eq!(
            bound.repository().unwrap().repository.working_tree(),
            same.working_tree()
        );
        for held in [&other, &other_scope] {
            assert!(matches!(
                context(&prepared).within(held).unwrap_err(),
                OperationFailure::Internal
            ));
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ContextKind {
    DataFree,
    Repository,
    Scoped,
    Scope,
}
#[derive(Debug, Clone)]
pub enum RequestedContext {
    Repository(RepositoryTarget),
    Scoped(RepositoryContext),
    Scope(RepositoryScope),
}
