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
        export { $($export_variant:ident: $export_type:ty, $export_field:ident, $export_path:ident, $export_suffix:literal, $export_table:literal, [$($export_node:tt)*], $export_reader:ident, [$($export_closed:tt)*], $export_id:ident, [$($export_loader:tt)*], [$($export_catalog:tt)*];)* }
        canonical { $($canonical_variant:ident: $canonical_type:ty, $canonical_field:ident, $canonical_path:ident, $canonical_suffix:literal, $canonical_table:literal, [$($canonical_node:tt)*], $canonical_reader:ident, [$($canonical_closed:tt)*], $canonical_id:ident, [$($canonical_loader:tt)*], [$($canonical_catalog:tt)*];)* }
        bindings { $($binding_variant:ident: $binding_type:ty, $binding_field:ident, $binding_path:ident, $binding_suffix:literal, $binding_table:literal, [$($binding_node:tt)*], $binding_reader:ident, [$($binding_closed:tt)*], $binding_id:ident, [$($binding_loader:tt)*], [$($binding_catalog:tt)*];)* }
        internal { $($internal_variant:ident: $internal_type:ty, $internal_field:ident, $internal_path:ident, $internal_suffix:literal, $internal_table:literal, [$($internal_node:tt)*], $internal_reader:ident, [$($internal_closed:tt)*], $internal_id:ident, [$($internal_loader:tt)*], [$($internal_catalog:tt)*];)* }
    ) => {
        $(guarded_reader!($export_type, $export_reader);)*
        $(guarded_reader!($canonical_type, $canonical_reader);)*
        $(guarded_reader!($binding_type, $binding_reader);)*
        $(guarded_reader!($internal_type, $internal_reader);)*
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
