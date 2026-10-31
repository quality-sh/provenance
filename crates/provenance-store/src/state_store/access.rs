use super::{Access, RecordStamp, StateStore};
use crate::layout::ProvenanceLayout;
use crate::publication::{PublicationGuard, StageLock, WritePermissions};
use crate::repository::RepositoryIdentity;
use crate::storage::{StateKey, StateView};
use camino::{Utf8Path, Utf8PathBuf};
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
    /// creates no path. A record write, a write permission and a staged
    /// publication through it refuse.
    #[rule("rule_read_only_store_refuses_writes")]
    pub(crate) const fn read_only(layout: ProvenanceLayout) -> Self {
        Self {
            identity: None,
            layout,
            access: Access::ReadOnly,
            record_stamp: RecordStamp::ResolveFromRepository,
        }
    }

    /// A store that reads one view of the canonical state of `layout`. It
    /// reads no file and refuses every write and stage.
    pub(crate) const fn over_view(
        identity: RepositoryIdentity,
        layout: ProvenanceLayout,
        view: Arc<StateView>,
    ) -> Self {
        Self {
            identity: Some(identity),
            layout,
            access: Access::View(view),
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
            Access::Section { .. }
            | Access::StagedSection { .. }
            | Access::ReadOnly
            | Access::View(_) => operation(),
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

/// The kind of entry that [`StateStore::state_keys_under`] lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StateEntry {
    File,
    Directory,
}

impl StateStore {
    /// The bytes of the state file at `path`, or `None` when it does not
    /// exist. A view store answers from its view; any other store reads the
    /// file.
    pub(crate) fn state_bytes(&self, path: &Utf8Path) -> anyhow::Result<Option<Vec<u8>>> {
        match &self.access {
            Access::View(view) => Ok(view.get(&self.view_key(path)?).map(<[u8]>::to_vec)),
            _ => match std::fs::read(path) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error.into()),
            },
        }
    }

    /// Like [`Self::state_bytes`], as UTF-8 text.
    pub(crate) fn state_text(&self, path: &Utf8Path) -> anyhow::Result<Option<String>> {
        match &self.access {
            Access::View(_) => self
                .state_bytes(path)?
                .map(|bytes| String::from_utf8(bytes).map_err(|_| invalid_text(path)))
                .transpose(),
            _ => match std::fs::read_to_string(path) {
                Ok(text) => Ok(Some(text)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(error.into()),
            },
        }
    }

    /// The bytes of the state file at `path`, which must exist.
    pub(crate) fn read_state(&self, path: &Utf8Path) -> anyhow::Result<Vec<u8>> {
        match &self.access {
            Access::View(_) => self.state_bytes(path)?.ok_or_else(|| not_found(path)),
            _ => Ok(std::fs::read(path)?),
        }
    }

    /// Like [`Self::read_state`], as UTF-8 text.
    pub(crate) fn read_state_text(&self, path: &Utf8Path) -> anyhow::Result<String> {
        match &self.access {
            Access::View(_) => self.state_text(path)?.ok_or_else(|| not_found(path)),
            _ => Ok(std::fs::read_to_string(path)?),
        }
    }

    /// Whether the state file at `path` exists.
    pub(crate) fn state_exists(&self, path: &Utf8Path) -> anyhow::Result<bool> {
        match &self.access {
            Access::View(view) => Ok(view.get(&self.view_key(path)?).is_some()),
            _ => Ok(path.exists()),
        }
    }

    /// The names of the entries of `kind` directly below the state directory
    /// `directory`, sorted. An entry of that kind with a name that is not
    /// UTF-8 is refused with `non_utf8` of its path.
    pub(crate) fn state_keys_under(
        &self,
        directory: &Utf8Path,
        kind: StateEntry,
        non_utf8: impl Fn(&std::path::Path) -> anyhow::Error,
    ) -> anyhow::Result<Vec<String>> {
        if let Access::View(view) = &self.access {
            let prefix = self.view_prefix(directory)?;
            return Ok(view
                .children(&prefix)
                .into_iter()
                .filter(|(_, below)| *below == (kind == StateEntry::Directory))
                .map(|(name, _)| name)
                .collect());
        }
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let matches = match kind {
                StateEntry::File => file_type.is_file(),
                StateEntry::Directory => file_type.is_dir(),
            };
            if matches {
                names.push(
                    entry
                        .file_name()
                        .into_string()
                        .map_err(|_| non_utf8(&entry.path()))?,
                );
            }
        }
        names.sort();
        Ok(names)
    }

    fn view_key(&self, path: &Utf8Path) -> anyhow::Result<StateKey> {
        Ok(StateKey::new(self.view_prefix(path)?)?)
    }

    /// The path below the state directory with `/` between its parts. A path
    /// outside the state directory is not canonical state and is refused.
    fn view_prefix(&self, path: &Utf8Path) -> anyhow::Result<String> {
        let state = self.layout.state_dir();
        let relative: Utf8PathBuf = path
            .strip_prefix(&state)
            .map_err(|_| {
                anyhow::anyhow!("{path} is not canonical state, so a view does not hold it")
            })?
            .to_path_buf();
        Ok(relative.iter().collect::<Vec<_>>().join("/"))
    }
}

fn not_found(path: &Utf8Path) -> anyhow::Error {
    anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::NotFound))
        .context(format!("read {path}"))
}

fn invalid_text(path: &Utf8Path) -> anyhow::Error {
    anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::InvalidData))
        .context(format!("read {path}: the file is not UTF-8"))
}
