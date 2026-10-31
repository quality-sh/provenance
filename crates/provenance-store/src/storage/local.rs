//! The local-disk adapter: canonical state as files under `.provenance/state`.

use super::{
    Atomicity, PublishFailure, Revision, StateKey, StateStorage, StateView, StorageError, WriteSet,
};
use crate::layout::ProvenanceLayout;
use crate::shards;
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

    /// Returns the revision of the whole tree after the write. The tree is
    /// the manifest and each shard of a scope, as [`Self::read_view`] reads
    /// them; no other entry under `.provenance/state` is in the revision.
    /// With a base, the adapter has the tree from its read under the lock.
    /// With no base, it reads and hashes the tree under the lock after the
    /// write.
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
            bytes
                .map_or_else(
                    || remove_file(&path),
                    |bytes| write_file_atomic(&path, bytes),
                )
                .map_err(PublishFailure::Started)?;
        }
        base_tree.map_or_else(
            || {
                self.read_view()
                    .map(|view| view.revision().clone())
                    .map_err(|error| PublishFailure::Started(error.into()))
            },
            |mut tree| {
                writes.apply_to(&mut tree);
                Ok(Revision::of(&tree))
            },
        )
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

/// The manifest and each shard below `directory`, keyed by `prefix` and
/// their path below it. The walk enters only a directory that can hold the
/// manifest or a shard. Any other entry is not canonical state, and the view
/// leaves it out: a name that is not UTF-8, a symbolic link, a special
/// entry, or a file that is not the manifest or a shard. A shard that the
/// process cannot read is an error.
fn read_tree(
    directory: &Utf8Path,
    prefix: &str,
    entries: &mut BTreeMap<StateKey, Vec<u8>>,
) -> Result<(), StorageError> {
    for entry in std::fs::read_dir(directory).map_err(read_error(directory))? {
        let entry = entry.map_err(read_error(directory))?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let path = directory.join(&name);
        let key = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let kind = entry.file_type().map_err(read_error(&path))?;
        if kind.is_dir() && shards::can_hold_canonical_keys(&key) {
            read_tree(&path, &key, entries)?;
        } else if kind.is_file() && shards::is_canonical_key(&key) {
            let bytes = std::fs::read(&path).map_err(read_error(&path))?;
            entries.insert(StateKey::new(key)?, bytes);
        }
    }
    Ok(())
}

fn read_error(path: &Utf8Path) -> impl Fn(std::io::Error) -> StorageError + '_ {
    move |error| StorageError::Read(anyhow::Error::new(error).context(format!("read {path}")))
}
