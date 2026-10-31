//! The local-disk adapter: canonical state as files under `.provenance/state`.

use super::{
    Atomicity, PublishFailure, Revision, StateKey, StateStorage, StateView, StorageError, WriteSet,
};
use crate::layout::ProvenanceLayout;
use camino::Utf8Path;
use std::collections::BTreeMap;
use std::io::Write;

/// Canonical state in the files of one state directory.
///
/// The engine calls this adapter in the publication section of its layout,
/// after recovery, so no publication runs while it reads or writes, and
/// publishes are serialized. The adapter takes no lock itself: a store that
/// holds the section takes no second lock.
///
/// Each key lands with a temporary file and a rename, so one key lands
/// completely or not at all. The keys of a set land one by one, in key
/// order. The adapter refuses a whole set of more than one key until the
/// staged publication path moves behind the port.
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

    /// Returns the revision of the whole tree after the write. With a base,
    /// the adapter has the tree from its read under the lock. With no base,
    /// it reads and hashes the whole tree under the lock after the write.
    fn publish(&self, writes: WriteSet) -> Result<Revision, PublishFailure> {
        writes.ensure_disjoint()?;
        if writes.atomicity() == Atomicity::WholeSet && writes.key_count() > 1 {
            return Err(PublishFailure::Refused(anyhow::anyhow!(
                "the local adapter cannot land a whole set of {} keys",
                writes.key_count()
            )));
        }
        let base_tree = match writes.base() {
            Some(base) => {
                let view = self
                    .read_view()
                    .map_err(|error| PublishFailure::Refused(error.into()))?;
                if view.revision() != base {
                    return Err(PublishFailure::BaseMoved {
                        base: base.clone(),
                        current: view.revision().clone(),
                    });
                }
                Some(view.into_entries())
            }
            None => None,
        };
        let mut changes: BTreeMap<&StateKey, Option<&[u8]>> = BTreeMap::new();
        changes.extend(writes.deletes().iter().map(|key| (key, None)));
        changes.extend(
            writes
                .puts()
                .iter()
                .map(|(key, bytes)| (key, Some(bytes.as_slice()))),
        );
        for (key, bytes) in changes {
            let path = self.layout.state_dir().join(key.as_str());
            match bytes {
                Some(bytes) => write_file_atomic(&path, bytes),
                None => remove_file(&path),
            }
            .map_err(PublishFailure::Started)?;
        }
        match base_tree {
            Some(mut tree) => {
                writes.apply_to(&mut tree);
                Ok(Revision::of(&tree))
            }
            None => self
                .read_view()
                .map(|view| view.revision().clone())
                .map_err(|error| PublishFailure::Started(error.into())),
        }
    }
}

/// Replaces the file at `path` with `bytes` through a temporary file in the
/// same directory and a rename.
pub(crate) fn write_file_atomic(path: &Utf8Path, bytes: &[u8]) -> anyhow::Result<()> {
    let Some(parent) = path.parent() else {
        anyhow::bail!("{path} has no parent directory");
    };
    std::fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(crate::long_path::extended(parent)?)?;
    temp.write_all(bytes)?;
    temp.persist(crate::long_path::extended(path)?)?;
    Ok(())
}

fn remove_file(path: &Utf8Path) -> anyhow::Result<()> {
    match std::fs::remove_file(crate::long_path::extended(path)?) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(anyhow::Error::new(error).context(format!("remove {path}")))
        }
        _ => Ok(()),
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
