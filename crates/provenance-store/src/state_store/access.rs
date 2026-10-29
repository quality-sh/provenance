use super::{Access, RecordStamp, StateStore};
use crate::layout::ProvenanceLayout;
use crate::publication::{with_repository_publication, PublicationGuard, WritePermissions};
use crate::repository::RepositoryIdentity;
use camino::Utf8Path;
use provenance_macros::{rule, verifies};
use std::marker::PhantomData;

mod guarded_readers;

#[cfg(test)]
mod tests;

impl StateStore {
    /// A layout-level store with no repository identity.
    pub(crate) const fn new(layout: ProvenanceLayout) -> Self {
        Self {
            identity: None,
            layout,
            access: Access::Lock,
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store over the live state of an opened repository.
    pub(crate) const fn live(identity: RepositoryIdentity, layout: ProvenanceLayout) -> Self {
        Self {
            identity: Some(identity),
            layout,
            access: Access::Lock,
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store over a staged state root. It keeps the identity of the live
    /// store and takes no lock, because the live publication section holds it.
    pub(crate) const fn staged_view(
        identity: Option<RepositoryIdentity>,
        layout: ProvenanceLayout,
        record_stamp: RecordStamp,
    ) -> Self {
        Self {
            identity,
            layout,
            access: Access::Held {
                permissions: WritePermissions::none(),
            },
            record_stamp,
        }
    }

    /// The identity of the repository the store belongs to. A layout-level
    /// store has none.
    pub const fn repository(&self) -> Option<&RepositoryIdentity> {
        self.identity.as_ref()
    }

    /// The identity, for a staged view of this store.
    pub(crate) fn identity_for_stage(&self) -> Option<RepositoryIdentity> {
        self.identity.clone()
    }

    /// Runs `run` with this staged store and a permission to change the
    /// enrolled rows of `id` (or of every id, `*`) in `paths`. A store that
    /// takes its own lock is refused: a review write runs in a stage.
    pub(crate) fn permitting<R>(
        &self,
        paths: &[&Utf8Path],
        id: &str,
        run: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let Access::Held { permissions } = &self.access else {
            anyhow::bail!("a review write permission needs a staged store");
        };
        let permitted = Self {
            identity: self.identity.clone(),
            layout: self.layout.clone(),
            access: Access::Held {
                permissions: permissions.with(paths, id),
            },
            record_stamp: self.record_stamp.clone(),
        };
        run(&permitted)
    }

    /// Whether this store may change the enrolled rows of `id` in `path`.
    pub(crate) fn permits(&self, path: &Utf8Path, id: &str) -> bool {
        self.permissions().allows(path, id)
    }

    /// Whether this store may change enrolled rows of any id in `path`.
    pub(crate) fn permits_path(&self, path: &Utf8Path) -> bool {
        self.permissions().allows_path(path)
    }

    pub(crate) fn permissions(&self) -> &WritePermissions {
        static NONE: WritePermissions = WritePermissions::none();
        match &self.access {
            Access::Lock => &NONE,
            Access::Held { permissions } => permissions,
        }
    }
}

/// A read-only store that borrows its repository publication guard.
#[verifies("rule_store_under_guard_takes_no_second_lock", construction)]
#[verifies("rule_guarded_reads_use_guard_repository", construction)]
pub struct GuardedStore<'g> {
    store: StateStore,
    _guard: PhantomData<&'g PublicationGuard>,
}

impl StateStore {
    /// Reads the repository of a held publication guard without a second lock.
    ///
    /// ```compile_fail
    /// use provenance_store::publication::PublicationGuard;
    /// use provenance_store::repository::Repository;
    /// use provenance_store::state_store::StateStore;
    /// let repository = Repository::open("repo", "default").unwrap();
    /// let forged = PublicationGuard { _lock: None, repository };
    /// let _ = StateStore::under_guard(&forged);
    /// ```
    pub fn under_guard(guard: &PublicationGuard) -> GuardedStore<'_> {
        GuardedStore {
            store: Self::staged_view(
                Some(guard.repository().identity().clone()),
                guard.repository().layout().clone(),
                super::RecordStamp::ResolveFromRepository,
            ),
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

    /// Uses the publication lock for writes. A store under a held section
    /// writes without a second lock.
    pub fn with_repository_publication<R>(
        &self,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        self.publication_access(&self.layout, operation)
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
            Access::Held { .. } => operation(),
        }
    }
}
