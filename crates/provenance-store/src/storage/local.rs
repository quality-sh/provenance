//! The local-disk adapter: canonical state as files under `.provenance/state`.

use super::{StateKey, StateStorage, StateView, StorageError};
use crate::layout::ProvenanceLayout;
use camino::Utf8Path;
use std::collections::BTreeMap;

/// Canonical state in the files of one state directory.
///
/// The engine calls this adapter in the publication section of its layout,
/// after recovery, so no publication runs while it reads. The adapter takes
/// no lock itself: a store that holds the section takes no second lock.
#[derive(Clone, Debug)]
pub struct LocalStateStorage {
    layout: ProvenanceLayout,
}

impl LocalStateStorage {
    pub const fn new(layout: ProvenanceLayout) -> Self {
        Self { layout }
    }
}

impl StateStorage for LocalStateStorage {
    fn read_view(&self) -> Result<StateView, StorageError> {
        let mut entries = BTreeMap::new();
        let state = self.layout.state_dir();
        if state.try_exists().map_err(read_error(&state))? {
            read_tree(&state, "", &mut entries)?;
        }
        Ok(StateView::new(entries))
    }
}

/// Every file below `directory`, keyed by `prefix` and its path below it.
fn read_tree(
    directory: &Utf8Path,
    prefix: &str,
    entries: &mut BTreeMap<StateKey, Vec<u8>>,
) -> Result<(), StorageError> {
    for entry in directory.read_dir_utf8().map_err(read_error(directory))? {
        let entry = entry.map_err(read_error(directory))?;
        let path = entry.path();
        let key = if prefix.is_empty() {
            entry.file_name().to_owned()
        } else {
            format!("{prefix}/{}", entry.file_name())
        };
        let kind = entry.file_type().map_err(read_error(path))?;
        if kind.is_dir() {
            read_tree(path, &key, entries)?;
        } else if kind.is_file() {
            let bytes = std::fs::read(path).map_err(read_error(path))?;
            entries.insert(StateKey::new(key)?, bytes);
        } else {
            return Err(StorageError::Read(anyhow::anyhow!(
                "state path {path} is not a file or a directory"
            )));
        }
    }
    Ok(())
}

fn read_error(path: &Utf8Path) -> impl Fn(std::io::Error) -> StorageError + '_ {
    move |error| StorageError::Read(anyhow::Error::new(error).context(format!("read {path}")))
}
