//! The binding of one local repository alias and scope to its files.
use crate::{
    layout::ProvenanceLayout,
    operations::{
        catalog::{
            ContextResolver, ExecutionNeed, ExecutionNeeds, PreparedContext, PreparedRead,
            PreparedRepository, PreparedScope, RequestedContext,
        },
        files::read_repository_file,
        read_policy::ReadPolicy,
    },
    settings::Settings,
};
use provenance_core::{
    protocol::failure::{InvalidInputReason, OperationFailure},
    Manifest, ScopeId,
};
use std::path::Path;

/// One existing repository bound under an alias, with one granted scope.
pub struct LocalRepository {
    layout: ProvenanceLayout,
    root: String,
    repository: String,
    scope: ScopeId,
}

impl LocalRepository {
    /// Bind an alias and scope to one existing repository whose manifest declares the scope.
    pub fn new(root: &Path, repository: &str, scope: &str) -> Result<Self, OperationFailure> {
        let invalid = || OperationFailure::InvalidInput {
            field: None,
            reason: InvalidInputReason::InvalidValue,
        };
        if repository.is_empty()
            || repository.contains(['/', '\\'])
            || repository.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        let root = root.canonicalize().map_err(|_| invalid())?;
        if !root.is_dir() {
            return Err(invalid());
        }
        let root = root.to_str().ok_or_else(invalid)?.to_owned();
        let binding = Self {
            layout: ProvenanceLayout::new(root.as_str()),
            root,
            repository: repository.to_owned(),
            scope: ScopeId::new(scope).map_err(|_| invalid())?,
        };
        binding.check_scope()?;
        Ok(binding)
    }

    /// The bound alias.
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// The bound scope.
    pub const fn scope(&self) -> &ScopeId {
        &self.scope
    }

    /// Read disposition actors from the repository bound at startup.
    pub fn disposition_actor_ids(&self) -> Result<Vec<String>, OperationFailure> {
        Ok(self.manifest()?.disposition_actor_ids)
    }

    /// The alias and scope compare that both resolver steps apply; no repository read.
    fn check_identity(
        &self,
        repository: &str,
        scope: Option<&str>,
    ) -> Result<(), OperationFailure> {
        if repository != self.repository {
            return Err(OperationFailure::UnknownTarget);
        }
        if scope.is_some_and(|scope| scope != self.scope.as_str()) {
            return Err(OperationFailure::AccessDenied);
        }
        Ok(())
    }

    fn check_scope(&self) -> Result<(), OperationFailure> {
        let manifest = self.manifest()?;
        if !manifest.scopes.iter().any(|scope| scope.id == self.scope) {
            return Err(OperationFailure::UnknownScope);
        }
        Ok(())
    }

    fn manifest(&self) -> Result<Manifest, OperationFailure> {
        let bytes = read_repository_file(
            Path::new(&self.root),
            Path::new(".provenance/state/manifest.json"),
        )
        .map_err(|_| OperationFailure::Internal)?;
        serde_json::from_slice(&bytes).map_err(|_| OperationFailure::Internal)
    }
}

impl ContextResolver for LocalRepository {
    fn authorize(
        &self,
        _: &'static str,
        context: &RequestedContext,
    ) -> Result<(), OperationFailure> {
        let (repository, scope) = match context {
            RequestedContext::Repository(context) => (&context.repository, None),
            RequestedContext::Scope(context) => (&context.repository, Some(&context.scope)),
            RequestedContext::Scoped(context) => (&context.repository, Some(&context.scope)),
        };
        self.check_identity(repository, scope.map(String::as_str))
    }

    fn prepare(
        &self,
        _: &'static str,
        context: RequestedContext,
        needs: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        let (repository, scope, freshness) = match context {
            RequestedContext::Repository(context) => (context.repository, None, None),
            RequestedContext::Scope(context) => (context.repository, Some(context.scope), None),
            RequestedContext::Scoped(context) => {
                (context.repository, Some(context.scope), context.freshness)
            }
        };
        self.check_identity(&repository, scope.as_deref())?;
        let root = self.root.clone().into();
        if scope.is_none() {
            // Repository-only operations do not use the scope grant or scope storage.
            return Ok(PreparedContext::for_repository(PreparedRepository {
                root,
                requested_target: repository,
            }));
        }
        // Validate the grant before settings, recovery, or projection access.
        self.check_scope()?;
        if !needs.contains(&ExecutionNeed::ProjectionMaintenance) {
            return Ok(PreparedContext::for_scope(PreparedScope {
                root,
                scope: self.scope.clone(),
                requested_target: repository,
            }));
        }
        let settings = Settings::load(&self.layout).map_err(|_| OperationFailure::Internal)?;
        Ok(PreparedContext::read(PreparedRead {
            root,
            scope: self.scope.clone(),
            policy: ReadPolicy::resolve(&settings, freshness),
            requested_target: repository,
            external: true,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_core::{
        protocol::repository::{RepositoryContext, RepositoryScope, RepositoryTarget},
        RepoPathPrefix,
    };

    fn repository() -> (tempfile::TempDir, LocalRepository) {
        let dir = tempfile::tempdir().unwrap();
        let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
        std::fs::create_dir_all(layout.state_dir()).unwrap();
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );
        std::fs::write(
            layout.manifest_path(),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let binding = LocalRepository::new(dir.path(), "native", "default").unwrap();
        (dir, binding)
    }

    #[test]
    fn authorize_refuses_another_alias_and_scope_without_reading() {
        let (dir, binding) = repository();
        std::fs::write(
            dir.path().join(".provenance/state/manifest.json"),
            "not a manifest",
        )
        .unwrap();
        assert_eq!(
            binding.disposition_actor_ids(),
            Err(OperationFailure::Internal)
        );

        let other_alias = RequestedContext::Repository(RepositoryTarget {
            repository: "other".into(),
        });
        assert_eq!(
            binding.authorize("info", &other_alias),
            Err(OperationFailure::UnknownTarget)
        );
        let other_scope = RequestedContext::Scope(RepositoryScope {
            repository: "native".into(),
            scope: "other".into(),
        });
        assert_eq!(
            binding.authorize("write-discussion", &other_scope),
            Err(OperationFailure::AccessDenied)
        );
        let bound = RequestedContext::Scoped(RepositoryContext {
            repository: "native".into(),
            scope: "default".into(),
            freshness: None,
        });
        assert_eq!(binding.authorize("search", &bound), Ok(()));
    }
}
