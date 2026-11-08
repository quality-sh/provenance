use super::{Access, RecordStamp, StateStore, Storage};
use crate::layout::ProvenanceLayout;
use crate::publication::{PublicationGuard, StageLock, WritePermissions};
use crate::repository::RepositoryIdentity;
use crate::storage::local::LocalStateStorage;
use crate::storage::{SectionLock, StateStorage, StateView};
use camino::Utf8Path;
use provenance_macros::{rule, verifies};
use std::marker::PhantomData;
use std::sync::Arc;

mod guarded_readers;
mod section_view;

pub use section_view::{SectionView, StateEntry};

#[cfg(test)]
mod tests;

impl StateStore {
    /// A layout-level store with no repository identity.
    pub(crate) const fn new(layout: ProvenanceLayout) -> Self {
        Self {
            identity: None,
            storage: Storage::Layout,
            layout,
            access: Access::Lock,
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store over the live state of an opened repository. A store of the
    /// repository of a section guard holds that section while `section` is
    /// open; any other store takes `lock` for each read and write.
    pub(crate) fn live(
        identity: RepositoryIdentity,
        layout: ProvenanceLayout,
        section: Option<crate::publication::SectionHandle>,
        storage: Arc<dyn StateStorage>,
        lock: Arc<dyn SectionLock>,
    ) -> Self {
        Self {
            identity: Some(identity),
            storage: Storage::Shared { storage, lock },
            layout,
            access: section.map_or(Access::Lock, |until| Access::Section {
                view: Some(until.view()),
                until: Some(until),
            }),
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store that holds the live section of `layout`, handed to the
    /// closure of a layout-level section.
    pub(crate) const fn in_section(layout: ProvenanceLayout) -> Self {
        Self {
            identity: None,
            storage: Storage::Layout,
            layout,
            access: Access::Section {
                until: None,
                view: None,
            },
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store over `layout` that takes no lock, runs no recovery and
    /// creates no path. A record write, a write permission and a staged
    /// publication through it refuse.
    #[rule("rule_read_only_store_refuses_writes")]
    pub(crate) const fn read_only(layout: ProvenanceLayout) -> Self {
        Self {
            identity: None,
            storage: Storage::Layout,
            layout,
            access: Access::ReadOnly,
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store that reads one view of the canonical state of `layout`. It
    /// reads no file and refuses every write and stage.
    pub(crate) fn over_view(
        identity: RepositoryIdentity,
        layout: ProvenanceLayout,
        view: Arc<StateView>,
        storage: Arc<dyn StateStorage>,
        lock: Arc<dyn SectionLock>,
    ) -> Self {
        Self {
            identity: Some(identity),
            storage: Storage::Shared { storage, lock },
            layout,
            access: Access::View(view),
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store over a stage of the state that this store holds: the live
    /// section, or the staged section of an outer stage. The staged store
    /// keeps the identity and the layout of this store, so its paths name
    /// the live state. It reads the overlay of the stage, and each write
    /// lands on that overlay. Its reads and writes take the lock that the
    /// stage owns, not the section lock, because this store holds that.
    pub(crate) fn begin_stage(&self, record_stamp: RecordStamp) -> anyhow::Result<Self> {
        let base = if let Access::StagedSection { stage, .. } = &self.access {
            stage.read(Clone::clone)
        } else {
            self.ensure_holds_live_section()?;
            match self.section_view() {
                Some(view) => view.read(self.storage().as_ref(), Clone::clone)?,
                None => self.storage().read_view()?,
            }
        };
        Ok(Self {
            identity: self.identity.clone(),
            storage: self.storage.clone(),
            layout: self.layout.clone(),
            access: Access::Staged {
                permissions: WritePermissions::none(),
                stage: StageLock::new(self.layout.root(), base),
            },
            record_stamp,
        })
    }

    /// A staged store that reads and writes the files of the staged state
    /// root `layout`, for tests of the staged writers.
    #[cfg(test)]
    pub(crate) fn staged_view(
        identity: Option<RepositoryIdentity>,
        layout: ProvenanceLayout,
        record_stamp: RecordStamp,
    ) -> Self {
        Self {
            identity,
            storage: Storage::Layout,
            access: Access::Staged {
                permissions: WritePermissions::none(),
                stage: StageLock::over_files(layout.root()),
            },
            layout,
            record_stamp,
        }
    }

    /// The changes of the stage of this staged store, as one whole set.
    pub(crate) fn stage_changes(&self) -> anyhow::Result<crate::storage::WriteSet> {
        match &self.access {
            Access::Staged { stage, .. } => Ok(stage.changes()),
            _ => anyhow::bail!("only a staged store has stage changes"),
        }
    }

    /// The stage of this store, if it is a staged store.
    pub(crate) fn stage(&self) -> Option<&StageLock> {
        match &self.access {
            Access::Staged { stage, .. } | Access::StagedSection { stage, .. } => Some(stage),
            _ => None,
        }
    }

    /// Refuses when a staged section of this store panicked.
    pub(crate) fn ensure_stage_intact(&self) -> anyhow::Result<()> {
        match &self.access {
            Access::Staged { stage, .. } => stage.ensure_intact(),
            _ => Ok(()),
        }
    }

    /// Refuses unless this store holds the live section. A view store
    /// refuses as it does for every write.
    pub(crate) fn ensure_holds_live_section(&self) -> anyhow::Result<()> {
        self.ensure_not_view()?;
        anyhow::ensure!(
            matches!(self.access, Access::Section { .. }),
            "a staged write needs the store that holds the live section"
        );
        self.ensure_section_open()
    }

    /// Refuses when this store holds the live section or reads with no lock
    /// and a stage body of its repository runs on this thread, or when it came
    /// from the repository of a section guard and that section has ended.
    fn ensure_section_open(&self) -> anyhow::Result<()> {
        match &self.access {
            Access::Section { until, .. } => {
                crate::publication::refuse_held_access_during_stage(&self.layout)?;
                if let Some(until) = until {
                    until.ensure_open()?;
                }
            }
            Access::ReadOnly => {
                crate::publication::refuse_held_access_during_stage(&self.layout)?;
            }
            Access::Lock
            | Access::Staged { .. }
            | Access::StagedSection { .. }
            | Access::View(_) => {}
        }
        Ok(())
    }

    /// The store that the section of this store hands its closure.
    fn section_store(&self, access: Access) -> Self {
        Self {
            identity: self.identity.clone(),
            storage: self.storage.clone(),
            layout: self.layout.clone(),
            access,
            record_stamp: self.record_stamp.clone(),
        }
    }

    /// The identity of the repository the store belongs to. A layout-level
    /// store has none.
    pub const fn repository(&self) -> Option<&RepositoryIdentity> {
        self.identity.as_ref()
    }

    /// The adapter that this store publishes through.
    pub(crate) fn storage(&self) -> Arc<dyn StateStorage> {
        match &self.storage {
            Storage::Layout => Arc::new(LocalStateStorage::new(self.layout.clone())),
            Storage::Shared { storage, .. } => Arc::clone(storage),
        }
    }

    /// Runs `run` with this staged store and a permission to change the
    /// enrolled rows of `id` (or of every id, `*`) in `paths`. A live store,
    /// also one that holds the section, is refused: a review write runs in a
    /// stage.
    pub(crate) fn permitting<R>(
        &self,
        paths: &[&Utf8Path],
        id: &str,
        run: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let access = match &self.access {
            Access::Staged { permissions, stage } => Access::Staged {
                permissions: permissions.with(paths, id),
                stage: Arc::clone(stage),
            },
            Access::StagedSection { permissions, stage } => Access::StagedSection {
                permissions: permissions.with(paths, id),
                stage: Arc::clone(stage),
            },
            Access::Lock | Access::Section { .. } | Access::ReadOnly | Access::View(_) => {
                anyhow::bail!("a review write permission needs a staged store")
            }
        };
        run(&self.section_store(access))
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
            Access::Lock | Access::Section { .. } | Access::ReadOnly | Access::View(_) => &NONE,
            Access::Staged { permissions, .. } | Access::StagedSection { permissions, .. } => {
                permissions
            }
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
            store: Self {
                identity: Some(guard.repository().identity().clone()),
                storage: Storage::Shared {
                    storage: Arc::clone(guard.repository().storage()),
                    lock: Arc::clone(guard.repository().section_lock()),
                },
                layout: guard.repository().layout().clone(),
                access: Access::Section {
                    until: guard.repository().section().cloned(),
                    view: guard
                        .repository()
                        .section()
                        .map(crate::publication::SectionHandle::view),
                },
                record_stamp: RecordStamp::ResolveFromRepository,
            },
            _guard: PhantomData,
        }
    }

    /// Runs `operation` on the state file at `path` in the section of this
    /// store, and hands it the store that holds the section.
    pub(super) fn state_path_access<R>(
        &self,
        path: &Utf8Path,
        operation: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        self.ensure_section_open()?;
        crate::test_probes::record_read(path);
        let Some(state_dir) = path.ancestors().find(|ancestor| {
            ancestor.file_name() == Some("state")
                && ancestor.parent().and_then(Utf8Path::file_name) == Some(".provenance")
        }) else {
            return operation(self);
        };
        let root = state_dir
            .parent()
            .and_then(Utf8Path::parent)
            .ok_or_else(|| anyhow::anyhow!("state path has no repository root"))?;
        match (&self.access, &self.storage) {
            (Access::Lock, Storage::Layout) => {
                crate::publication::with_repository_publication_checked(
                    &ProvenanceLayout::new(root),
                    || Ok(()),
                    || operation(self),
                )
            }
            (Access::Lock, Storage::Shared { .. }) => self.with_repository_publication(operation),
            (Access::Staged { stage, .. }, _) => stage.run(|| operation(self)),
            (
                Access::Section { .. }
                | Access::StagedSection { .. }
                | Access::ReadOnly
                | Access::View(_),
                _,
            ) => operation(self),
        }
    }

    /// Runs `operation` in the section of this store and hands it the store
    /// that holds the section. A live store takes the publication lock; a
    /// staged store takes the lock that its stage owns; a store that holds
    /// the section hands itself; a read-only store hands itself and takes no
    /// lock. Another store of the same repository on this
    /// thread returns [`PublicationHeld`](crate::publication::PublicationHeld).
    #[rule("rule_store_under_guard_takes_no_second_lock")]
    pub fn with_repository_publication<R>(
        &self,
        operation: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        match &self.access {
            Access::Lock => match &self.storage {
                Storage::Layout => crate::publication::with_repository_publication_checked(
                    &self.layout,
                    || Ok(()),
                    || {
                        operation(&self.section_store(Access::Section {
                            until: None,
                            view: None,
                        }))
                    },
                ),
                Storage::Shared { lock, .. } => {
                    crate::publication::with_section(&self.layout, lock.as_ref(), || {
                        operation(&self.section_store(Access::Section {
                            until: None,
                            view: Some(Arc::default()),
                        }))
                    })
                }
            },
            Access::Staged { permissions, stage } => stage.run(|| {
                operation(&self.section_store(Access::StagedSection {
                    permissions: permissions.clone(),
                    stage: Arc::clone(stage),
                }))
            }),
            Access::Section { .. } | Access::ReadOnly => {
                self.ensure_section_open()?;
                operation(self)
            }
            Access::StagedSection { .. } | Access::View(_) => operation(self),
        }
    }

    /// Refuses a write through a read-only store or a view store.
    pub(crate) fn ensure_writable(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !matches!(self.access, Access::ReadOnly),
            "a read-only store of {} cannot write",
            self.layout.root()
        );
        self.ensure_not_view()
    }

    fn ensure_not_view(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !matches!(self.access, Access::View(_)),
            "a view store of {} cannot write",
            self.layout.root()
        );
        Ok(())
    }

    pub(super) fn with_repository_read<R>(
        &self,
        operation: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        self.with_repository_publication(operation)
    }
}
