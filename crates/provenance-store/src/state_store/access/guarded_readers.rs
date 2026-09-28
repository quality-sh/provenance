use super::GuardedStore;
use provenance_core::{
    ImplementationBinding, Manifest, ProposalCard, Requirement, ScopeId, VerificationBinding,
};

macro_rules! guarded_reader {
    ($record:ty, list_requirements) => {};
    ($record:ty, validated_journal_entries) => {
        pub(crate) fn validated_journal_entries(
            &self,
            scope: &ScopeId,
        ) -> anyhow::Result<Vec<$record>> {
            self.store.validated_journal_entries(scope)
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
