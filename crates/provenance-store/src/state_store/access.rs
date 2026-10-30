use super::{Access, RecordStamp, StateStore};
use crate::layout::ProvenanceLayout;
use crate::publication::{PublicationGuard, StageLock, WritePermissions};
use crate::repository::RepositoryIdentity;
use camino::Utf8Path;
use provenance_macros::{rule, verifies};
use std::marker::PhantomData;
use std::sync::Arc;

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

    /// A store over the live state of an opened repository. A store of the
    /// repository of a section guard holds that section while `section` is
    /// open; any other store takes the publication lock.
    pub(crate) fn live(
        identity: RepositoryIdentity,
        layout: ProvenanceLayout,
        section: Option<crate::publication::SectionHandle>,
    ) -> Self {
        Self {
            identity: Some(identity),
            layout,
            access: section.map_or(Access::Lock, |until| Access::Section { until: Some(until) }),
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store that holds the live section of `layout`, handed to the
    /// closure of a layout-level section.
    pub(crate) const fn in_section(layout: ProvenanceLayout) -> Self {
        Self {
            identity: None,
            layout,
            access: Access::Section { until: None },
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store over `layout` that takes no lock, runs no recovery and
    /// creates no path. It refuses every write, because a write needs the
    /// publication section and this store enters none.
    #[rule("rule_read_only_store_refuses_writes")]
    pub(crate) const fn read_only(layout: ProvenanceLayout) -> Self {
        Self {
            identity: None,
            layout,
            access: Access::ReadOnly,
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store over a staged state root. It keeps the identity of the live
    /// store. Its reads and writes take the lock that the stage owns, not the
    /// publication lock, because the live section holds that.
    pub(crate) fn staged_view(
        identity: Option<RepositoryIdentity>,
        layout: ProvenanceLayout,
        record_stamp: RecordStamp,
    ) -> Self {
        let stage = StageLock::new(layout.root());
        Self {
            identity,
            layout,
            access: Access::Staged {
                permissions: WritePermissions::none(),
                stage,
            },
            record_stamp,
        }
    }

    /// Refuses when a staged section of this store panicked.
    pub(crate) fn ensure_stage_intact(&self) -> anyhow::Result<()> {
        match &self.access {
            Access::Staged { stage, .. } => stage.ensure_intact(),
            _ => Ok(()),
        }
    }

    /// Whether this store holds the section of a staged state root. Refuses
    /// a store that holds no section.
    pub(crate) fn holds_staged_section(&self) -> anyhow::Result<bool> {
        match &self.access {
            Access::StagedSection { .. } => Ok(true),
            _ => self.ensure_holds_live_section().map(|()| false),
        }
    }

    /// Refuses unless this store holds the live section.
    pub(crate) fn ensure_holds_live_section(&self) -> anyhow::Result<()> {
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
            Access::Section { until } => {
                crate::publication::refuse_held_access_during_stage(&self.layout)?;
                if let Some(until) = until {
                    until.ensure_open()?;
                }
            }
            Access::ReadOnly => {
                crate::publication::refuse_held_access_during_stage(&self.layout)?;
            }
            Access::Lock | Access::Staged { .. } | Access::StagedSection { .. } => {}
        }
        Ok(())
    }

    /// The store that the section of this store hands its closure.
    fn section_store(&self, access: Access) -> Self {
        Self {
            identity: self.identity.clone(),
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

    /// The identity, for a staged view of this store.
    pub(crate) fn identity_for_stage(&self) -> Option<RepositoryIdentity> {
        self.identity.clone()
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
            Access::StagedSection { permissions } => Access::StagedSection {
                permissions: permissions.with(paths, id),
            },
            Access::Lock | Access::Section { .. } | Access::ReadOnly => {
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
            Access::Lock | Access::Section { .. } | Access::ReadOnly => &NONE,
            Access::Staged { permissions, .. } | Access::StagedSection { permissions } => {
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
                layout: guard.repository().layout().clone(),
                access: Access::Section {
                    until: guard.repository().section().cloned(),
                },
                record_stamp: RecordStamp::ResolveFromRepository,
            },
            _guard: PhantomData,
        }
    }

    pub(super) fn state_path_access<R>(
        &self,
        path: &Utf8Path,
        operation: impl FnOnce() -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        self.ensure_section_open()?;
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
        match &self.access {
            Access::Lock => crate::publication::with_repository_publication_checked(
                &ProvenanceLayout::new(root),
                || Ok(()),
                operation,
            ),
            Access::Staged { stage, .. } => stage.run(operation),
            Access::Section { .. } | Access::StagedSection { .. } | Access::ReadOnly => operation(),
        }
    }

    /// Runs `operation` in the section of this store and hands it the store
    /// that holds the section. A live store takes the publication lock; a
    /// staged store takes the lock that its stage owns; a store that holds
    /// the section hands itself; a read-only store refuses. Another store of the same repository on this
    /// thread returns [`PublicationHeld`](crate::publication::PublicationHeld).
    #[rule("rule_store_under_guard_takes_no_second_lock")]
    pub fn with_repository_publication<R>(
        &self,
        operation: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        match &self.access {
            Access::Lock => crate::publication::with_repository_publication_checked(
                &self.layout,
                || Ok(()),
                || operation(&self.section_store(Access::Section { until: None })),
            ),
            Access::Staged { permissions, stage } => stage.run(|| {
                operation(&self.section_store(Access::StagedSection {
                    permissions: permissions.clone(),
                }))
            }),
            Access::Section { .. } => {
                self.ensure_section_open()?;
                operation(self)
            }
            Access::StagedSection { .. } => operation(self),
            Access::ReadOnly => anyhow::bail!(
                "a read-only store of {} cannot write or hold a publication section",
                self.layout.root()
            ),
        }
    }

    /// Runs a read in the section of this store. A read-only store reads with
    /// no section.
    pub(super) fn with_repository_read<R>(
        &self,
        operation: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        if let Access::ReadOnly = self.access {
            self.ensure_section_open()?;
            return operation(self);
        }
        self.with_repository_publication(operation)
    }
}
