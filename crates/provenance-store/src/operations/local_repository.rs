//! The binding of one opened repository to its alias.
use crate::{
    operations::{
        catalog::{
            ContextResolver, ExecutionNeed, ExecutionNeeds, PreparedContext, PreparedRead,
            PreparedRepository, PreparedScope, RequestedContext,
        },
        read_policy::ReadPolicy,
    },
    repository::Repository,
};
use provenance_core::{
    protocol::failure::{InvalidInputReason, OperationFailure},
    ScopeId,
};

/// One opened repository bound under an alias, with its scope as the one grant.
pub struct LocalRepository {
    repository: Repository,
    alias: String,
}

impl LocalRepository {
    /// Binds an alias to one opened repository.
    pub fn bind(repository: Repository, alias: &str) -> Result<Self, OperationFailure> {
        if alias.is_empty() || alias.contains(['/', '\\']) || alias.chars().any(char::is_control) {
            return Err(OperationFailure::InvalidInput {
                field: None,
                reason: InvalidInputReason::InvalidValue,
            });
        }
        Ok(Self {
            repository,
            alias: alias.to_owned(),
        })
    }

    /// The bound alias.
    pub fn repository(&self) -> &str {
        &self.alias
    }

    /// The bound scope.
    pub fn scope(&self) -> &ScopeId {
        self.repository.scope()
    }

    /// Read disposition actors from the repository bound at startup.
    pub fn disposition_actor_ids(&self) -> Result<Vec<String>, OperationFailure> {
        Ok(self
            .repository
            .manifest()
            .map_err(|_| OperationFailure::Internal)?
            .disposition_actor_ids)
    }

    /// The alias and scope compare that both resolver steps apply; no repository read.
    fn check_identity(
        &self,
        repository: &str,
        scope: Option<&str>,
    ) -> Result<(), OperationFailure> {
        if repository != self.alias {
            return Err(OperationFailure::UnknownTarget);
        }
        if scope.is_some_and(|scope| scope != self.scope().as_str()) {
            return Err(OperationFailure::AccessDenied);
        }
        Ok(())
    }

    /// The manifest must still declare the bound scope.
    fn check_scope(&self) -> Result<(), OperationFailure> {
        let manifest = self
            .repository
            .manifest()
            .map_err(|_| OperationFailure::Internal)?;
        if !manifest
            .scopes
            .iter()
            .any(|scope| &scope.id == self.scope())
        {
            return Err(OperationFailure::UnknownScope);
        }
        Ok(())
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
        if scope.is_none() {
            // Repository-only operations do not use the scope grant or scope storage.
            return Ok(PreparedContext::for_repository(PreparedRepository {
                repository: self.repository.clone(),
                requested_target: repository,
            }));
        }
        // Validate the grant before recovery or projection access.
        self.check_scope()?;
        if !needs.contains(&ExecutionNeed::ProjectionMaintenance) {
            return Ok(PreparedContext::for_scope(PreparedScope {
                repository: self.repository.clone(),
                requested_target: repository,
            }));
        }
        Ok(PreparedContext::read(PreparedRead {
            repository: self.repository.clone(),
            policy: ReadPolicy::resolve(self.repository.settings(), freshness),
            requested_target: repository,
            external: true,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::ProvenanceLayout;
    use provenance_core::{
        protocol::repository::{RepositoryContext, RepositoryScope, RepositoryTarget},
        Manifest, RepoPathPrefix,
    };
    use provenance_macros::verifies;

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
        let opened = Repository::open(dir.path(), "default").unwrap();
        let binding = LocalRepository::bind(opened, "native").unwrap();
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

    #[test]
    fn a_refused_open_on_a_directory_without_state_creates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Repository::open(dir.path(), "default").is_err());
        assert!(!dir.path().join(".provenance").exists());
    }

    /// The probe starts the host call while the publication holds its lock with the live
    /// state moved away. The call must wait for the publication and not fail.
    #[test]
    #[verifies("rule_preparation_reads_published_state", examples)]
    fn preparation_in_the_publication_gap_succeeds_after_the_publication() {
        let (dir, binding) = repository();
        let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
        let reader = std::rc::Rc::new(std::cell::RefCell::new(None));
        crate::test_probes::arm("state_after_backup_rename", {
            let reader = reader.clone();
            let mut binding = Some(binding);
            move || {
                let binding = binding.take().expect("one publication");
                let (done, finished) = std::sync::mpsc::channel();
                reader.replace(Some(std::thread::spawn(move || {
                    let context = RequestedContext::Scoped(RepositoryContext {
                        repository: "native".into(),
                        scope: "default".into(),
                        freshness: None,
                    });
                    let prepared = binding.prepare("search", context, &[]).map(|_| ());
                    let _ = done.send(());
                    prepared
                })));
                // A call that does not wait ends in the gap. A call that waits ends only
                // after the publication, so this wait times out.
                let _ = finished.recv_timeout(std::time::Duration::from_secs(1));
                Ok(())
            }
        });
        crate::publication::with_staged_state(&layout, false, |_| Ok(())).unwrap();
        crate::test_probes::disarm("state_after_backup_rename");
        let reader = reader.take().expect("the publication reached the gap");
        assert_eq!(reader.join().unwrap(), Ok(()));
    }
}
