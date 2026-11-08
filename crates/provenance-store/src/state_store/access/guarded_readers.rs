use super::GuardedStore;
use crate::repository::Repository;
use crate::state_store::StateStore;
use crate::storage::StateView;
use provenance_core::{
    ImplementationBinding, Manifest, ProposalCard, Requirement, ScopeId, VerificationBinding,
};
use std::sync::Arc;

macro_rules! guarded_reader {
    ($record:ty, list_requirements) => {};
    ($record:ty, validated_discussions) => {
        pub(crate) fn validated_discussions(
            &self,
            scope: &ScopeId,
        ) -> anyhow::Result<Vec<$record>> {
            self.store.validated_discussions(scope)
        }
    };
    ($record:ty, $reader:ident) => {
        pub fn $reader(&self, scope: &ScopeId) -> anyhow::Result<Vec<$record>> {
            self.store.$reader(scope)
        }
    };
}

macro_rules! define_guarded_family_readers {
    (
        $(
            $group:ident {
                $(
                    $variant:ident {
                        record: $record:ty,
                        field: $field:ident,
                        path: $path:ident,
                        node: [$($node:tt)*],
                        reader: $reader:ident,
                        $($rest:tt)*
                    };
                )*
            }
        )*
    ) => {
        $($(guarded_reader!($record, $reader);)*)*
    };
}

impl GuardedStore<'_> {
    pub fn manifest(&self) -> anyhow::Result<Manifest> {
        self.store.manifest()
    }

    pub fn list_scope_directories(&self) -> anyhow::Result<Vec<String>> {
        self.store.list_scope_directories()
    }

    pub fn list_requirements(&self, scope: &ScopeId) -> anyhow::Result<Vec<Requirement>> {
        self.store.list_requirements(scope)
    }

    crate::cache::record_families!(define_guarded_family_readers);

    pub fn active_verification_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<VerificationBinding>> {
        self.store.active_verification_bindings(scope)
    }

    pub fn active_implementation_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ImplementationBinding>> {
        self.store.active_implementation_bindings(scope)
    }

    pub fn list_proposal_definitions(&self, scope: &ScopeId) -> anyhow::Result<Vec<ProposalCard>> {
        self.store.list_proposal_definitions(scope)
    }

    /// The proposal cards of a scope that the caller validated under this guard.
    pub(crate) fn proposal_cards_of_validated_scope(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ProposalCard>> {
        self.store.proposal_cards_of_validated_scope(scope)
    }

    pub fn list_proposal_cards_with_actor_ids(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<Vec<ProposalCard>> {
        self.store
            .list_proposal_cards_with_actor_ids(scope, disposition_actor_ids)
    }

    pub fn validate_ideation_scope(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.store.validate_ideation_scope(scope)
    }

    pub fn validate_ideation_scope_with_actor_ids(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<()> {
        self.store
            .validate_ideation_scope_with_actor_ids(scope, disposition_actor_ids)
    }

    pub fn validate_graph_scope(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.store.validate_graph_scope(scope)
    }
}

impl GuardedStore<'_> {
    /// Runs `body` inside a read fence of `directories` below the scope
    /// directory of `scope`. A read outside them fails the result.
    pub(crate) fn fenced<R>(
        &self,
        scope: &ScopeId,
        directories: Vec<&'static str>,
        body: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let scope_dir = self.store.layout.scopes_dir().join(scope.as_str());
        crate::state_store::with_read_fence(scope_dir, directories, body)
    }
}

/// A store that reads one view of the canonical state and no file. Only
/// [`ViewStore::new`] makes one, and it always reads a view.
pub struct ViewStore(StateStore);

impl ViewStore {
    /// A store over `view`, a view of the canonical state of `repository`.
    pub fn new(repository: &Repository, view: Arc<StateView>) -> Self {
        Self(StateStore::over_view(
            repository.identity().clone(),
            repository.layout().clone(),
            view,
            Arc::clone(repository.storage()),
            Arc::clone(repository.section_lock()),
            repository.has_local_storage(),
        ))
    }
}

impl GuardedStore<'static> {
    /// A guarded store over a view that was read under a guard. It reads no
    /// file, so it needs no guard after the read.
    pub(crate) fn of_view(view: ViewStore) -> Self {
        GuardedStore {
            store: view.0,
            _guard: std::marker::PhantomData,
        }
    }
}
