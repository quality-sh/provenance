//! The view that one live section reads and writes, and the state reads of a
//! store, which answer from a view when the store has one.

use super::super::{Access, StateStore};
use crate::storage::{StateKey, StateStorage, StateView, WriteSet};
use camino::{Utf8Path, Utf8PathBuf};
use std::sync::{Mutex, PoisonError};

/// One view of canonical state for the life of one live section.
///
/// The first read in the section reads the view from the adapter; every
/// later read of the section answers from it. Each write set that the
/// section publishes is applied to the view, so the section reads its own
/// writes. A change that does not go through the adapter, such as the
/// install of a staged state, makes the section read the view again.
#[derive(Debug, Default)]
pub struct SectionView {
    view: Mutex<Option<StateView>>,
}

impl SectionView {
    /// Runs `read` on the view, reading it from `storage` first if the
    /// section has no view yet.
    pub fn read<R>(
        &self,
        storage: &dyn StateStorage,
        read: impl FnOnce(&StateView) -> R,
    ) -> anyhow::Result<R> {
        let mut held = self.view();
        let view = match held.take() {
            Some(view) => view,
            None => storage.read_view()?,
        };
        let result = read(&view);
        *held = Some(view);
        drop(held);
        Ok(result)
    }

    /// The revision of the view, if the section has read it.
    pub fn revision(&self) -> Option<crate::storage::Revision> {
        self.view().as_ref().map(|view| view.revision().clone())
    }

    /// Applies a write set that landed.
    pub fn apply(&self, writes: &WriteSet) {
        let mut view = self.view();
        if let Some(current) = view.take() {
            let mut entries = current.into_entries();
            writes.apply_to(&mut entries);
            *view = Some(StateView::new(entries));
        }
    }

    /// Drops the view, so the next read reads it again.
    pub fn forget(&self) {
        *self.view() = None;
    }

    fn view(&self) -> std::sync::MutexGuard<'_, Option<StateView>> {
        // Every change replaces the whole value, so a poisoned view is intact.
        self.view.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The kind of entry that [`StateStore::state_keys_under`] lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateEntry {
    File,
    Directory,
}

impl StateStore {
    /// Runs `read` on the view that this store reads, or returns `None` for
    /// a store that reads the files: a view store has its view, a staged
    /// store the overlay of its stage, and a store of an opened repository
    /// that holds the live section has the view of that section.
    fn with_view<R>(&self, read: impl FnOnce(&StateView) -> R) -> anyhow::Result<Option<R>> {
        match &self.access {
            Access::View(view) => Ok(Some(read(view))),
            Access::Staged { stage, .. } | Access::StagedSection { stage, .. }
                if !stage.reads_files() =>
            {
                Ok(Some(stage.read(read)))
            }
            Access::Section {
                view: Some(section),
                ..
            } => section.read(self.storage().as_ref(), read).map(Some),
            _ => Ok(None),
        }
    }

    /// The section view of this store, if it holds the live section of an
    /// opened repository.
    pub(crate) fn section_view(&self) -> Option<&SectionView> {
        match &self.access {
            Access::Section {
                view: Some(view), ..
            } => Some(view),
            _ => None,
        }
    }

    /// The bytes of the state file at `path`, or `None` when it does not
    /// exist. A store with a view answers from it; any other store reads the
    /// file.
    pub(crate) fn state_bytes(&self, path: &Utf8Path) -> anyhow::Result<Option<Vec<u8>>> {
        let key = self.state_key(path);
        if let Some(bytes) = self.with_view(|view| Ok(view.get(&key?).map(<[u8]>::to_vec)))? {
            return bytes;
        }
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Like [`Self::state_bytes`], as UTF-8 text.
    pub(crate) fn state_text(&self, path: &Utf8Path) -> anyhow::Result<Option<String>> {
        if self.with_view(|_| ())?.is_some() {
            return self
                .state_bytes(path)?
                .map(|bytes| String::from_utf8(bytes).map_err(|_| invalid_text(path)))
                .transpose();
        }
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// The bytes of the state file at `path`, which must exist.
    pub(crate) fn read_state(&self, path: &Utf8Path) -> anyhow::Result<Vec<u8>> {
        if self.with_view(|_| ())?.is_some() {
            return self.state_bytes(path)?.ok_or_else(|| not_found(path));
        }
        Ok(std::fs::read(path)?)
    }

    /// Like [`Self::read_state`], as UTF-8 text.
    pub(crate) fn read_state_text(&self, path: &Utf8Path) -> anyhow::Result<String> {
        if self.with_view(|_| ())?.is_some() {
            return self.state_text(path)?.ok_or_else(|| not_found(path));
        }
        Ok(std::fs::read_to_string(path)?)
    }

    /// Whether the state file at `path` exists.
    pub(crate) fn state_exists(&self, path: &Utf8Path) -> anyhow::Result<bool> {
        let key = self.state_key(path);
        self.with_view(|view| Ok(view.get(&key?).is_some()))?
            .unwrap_or_else(|| Ok(path.exists()))
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
        let prefix = self.view_prefix(directory);
        let listed = self.with_view(|view| {
            Ok::<_, anyhow::Error>(
                view.children(&prefix?)
                    .into_iter()
                    .filter(|(_, below)| *below == (kind == StateEntry::Directory))
                    .map(|(name, _)| name)
                    .collect(),
            )
        })?;
        if let Some(names) = listed {
            return names;
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

    /// The key of the state file at `path`.
    pub(crate) fn state_key(&self, path: &Utf8Path) -> anyhow::Result<StateKey> {
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
