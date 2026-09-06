use super::{Access, StateStore};
use crate::layout::ProvenanceLayout;
use crate::publication::{with_repository_publication, PublicationGuard};
use camino::Utf8Path;
use provenance_macros::rule;
use std::marker::PhantomData;
use std::ops::Deref;

/// A store that uses a publication guard for the lifetime of its reads.
///
/// Writes use the ordinary publication lock path and wait for a held lock.
/// To write after reading, clone the store and release the guard first.
pub struct GuardedStore<'g> {
    store: StateStore,
    _guard: PhantomData<&'g PublicationGuard>,
}

impl Deref for GuardedStore<'_> {
    type Target = StateStore;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl Clone for StateStore {
    fn clone(&self) -> Self {
        Self::new(self.layout.clone())
    }
}

impl StateStore {
    /// Reads the repository of a held publication guard without a second lock.
    ///
    /// ```compile_fail
    /// use provenance_store::layout::ProvenanceLayout;
    /// use provenance_store::publication::PublicationGuard;
    /// use provenance_store::state_store::StateStore;
    /// let layout = ProvenanceLayout::new("repo");
    /// let forged = PublicationGuard { _lock: None, layout };
    /// let _ = StateStore::under_guard(&forged);
    /// ```
    pub fn under_guard(guard: &PublicationGuard) -> GuardedStore<'_> {
        GuardedStore {
            store: Self {
                layout: guard.layout().clone(),
                access: Access::Held,
            },
            _guard: PhantomData,
        }
    }

    pub(super) fn state_path_access<R>(
        &self,
        path: &Utf8Path,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        crate::test_probes::record_read(path);
        let Some(state_dir) = path.ancestors().find(|ancestor| {
            ancestor.file_name() == Some("state")
                && ancestor.parent().and_then(Utf8Path::file_name) == Some(".provenance")
        }) else {
            return operation();
        };
        let root = state_dir
            .parent()
            .and_then(Utf8Path::parent)
            .ok_or_else(|| anyhow::anyhow!("state path has no repository root"))?;
        self.publication_access(&ProvenanceLayout::new(root), operation)
    }

    /// Uses the ordinary publication lock path for writes, including guarded stores.
    #[rule("rule_guarded_store_writes_take_publication_lock")]
    pub fn with_repository_publication<R>(
        &self,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        with_repository_publication(&self.layout, operation)
    }

    pub(super) fn with_repository_read<R>(
        &self,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        self.publication_access(&self.layout, operation)
    }

    /// A store under a held publication guard reads without a second lock.
    #[rule("rule_store_under_guard_takes_no_second_lock")]
    fn publication_access<R>(
        &self,
        layout: &ProvenanceLayout,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        match self.access {
            Access::Lock => with_repository_publication(layout, operation),
            Access::Held => operation(),
        }
    }
}
