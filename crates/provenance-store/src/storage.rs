//! The storage port: where canonical state lives, apart from the engine.
//!
//! The engine reads canonical state as one consistent view. A view holds the
//! manifest and every shard under `.provenance/state`, byte for byte, keyed
//! by the path below the state directory with `/` between its parts. No
//! other entry under `.provenance/state` is in a view. A revision digests
//! every key of a view and its bytes. Derived state, such as the
//! verification runs under `.provenance/cache`, is not canonical state and is
//! not in a view.
//!
//! The engine publishes a change as one write set. A write set states the
//! atomicity that the change needs, and may carry the view that the change
//! was decided on: its base.
//!
//! The port is synchronous: the engine runs where its storage is.
//!
//! The store, not an adapter, parses the state, keeps the raw lines of a
//! shard, guards each JSONL line with `guard_jsonl_lines` and checks write
//! permissions. An adapter keeps keys and bytes only.
//!
//! The port and the memory adapter build without the `host` feature. The
//! local-disk adapter needs it.

#[cfg(any(test, feature = "test-fixture"))]
pub mod contract;
#[cfg(feature = "host")]
pub mod local;
pub mod memory;

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

/// The path of one state file below the state directory, with `/` between
/// its parts.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateKey(String);

impl StateKey {
    /// Refuses an empty key, an empty, `.` or `..` part, a `\` and a NUL.
    pub fn new(key: impl Into<String>) -> Result<Self, StorageError> {
        let key = key.into();
        let reason = if key.is_empty() {
            Some("the key is empty")
        } else if key.contains('\\') || key.contains('\0') {
            Some("the key contains `\\` or NUL")
        } else if key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        {
            Some("the key has an empty, `.` or `..` part")
        } else {
            None
        };
        match reason {
            Some(reason) => Err(StorageError::InvalidKey { key, reason }),
            None => Ok(Self(key)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A digest of every key of a state tree and its bytes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Revision(String);

impl Revision {
    /// The revision of `entries`. Each key and its bytes go into the digest
    /// with their lengths, so no two trees share a revision by a shift of
    /// bytes between a key and its contents.
    pub fn of(entries: &BTreeMap<StateKey, Vec<u8>>) -> Self {
        // The probe only counts hashes in lib tests, and it cannot fail there.
        drop(crate::test_probes::at("state_tree_hashed"));
        let mut digest = Sha256::new();
        for (key, bytes) in entries {
            digest.update((key.0.len() as u64).to_le_bytes());
            digest.update(key.0.as_bytes());
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        Self(format!("sha256:{}", hex::encode(digest.finalize())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One consistent read of canonical state, held in memory.
///
/// The revision is a digest of every byte of the view. A view computes it
/// on the first call of [`Self::revision`], so a reader that does not ask
/// for it does not pay for it.
#[derive(Clone, Debug)]
pub struct StateView {
    revision: OnceLock<Revision>,
    entries: BTreeMap<StateKey, Vec<u8>>,
}

/// Two views are equal when their entries are equal. The revision is a
/// function of the entries.
impl PartialEq for StateView {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for StateView {}

impl StateView {
    pub const fn new(entries: BTreeMap<StateKey, Vec<u8>>) -> Self {
        Self {
            revision: OnceLock::new(),
            entries,
        }
    }

    pub fn revision(&self) -> &Revision {
        self.revision.get_or_init(|| Revision::of(&self.entries))
    }

    pub const fn entries(&self) -> &BTreeMap<StateKey, Vec<u8>> {
        &self.entries
    }

    pub fn into_entries(self) -> BTreeMap<StateKey, Vec<u8>> {
        self.entries
    }

    pub fn get(&self, key: &StateKey) -> Option<&[u8]> {
        self.entries.get(key).map(Vec::as_slice)
    }

    /// Lands `writes` on this view. The revision is computed again on the
    /// next call of [`Self::revision`].
    pub fn apply(&mut self, writes: &WriteSet) {
        writes.apply_to(&mut self.entries);
        self.revision = OnceLock::new();
    }

    /// The names directly below `directory` (a key prefix, or `""` for the
    /// state directory), sorted, each with `true` when it has keys below it.
    pub fn children(&self, directory: &str) -> Vec<(String, bool)> {
        let prefix = if directory.is_empty() {
            String::new()
        } else {
            format!("{directory}/")
        };
        let mut children: BTreeMap<String, bool> = BTreeMap::new();
        for key in self.entries.keys() {
            if let Some(rest) = key.0.strip_prefix(&prefix) {
                let (name, below) = rest
                    .split_once('/')
                    .map_or((rest, false), |(name, _)| (name, true));
                *children.entry(name.to_owned()).or_default() |= below;
            }
        }
        children.into_iter().collect()
    }
}

/// What a write set needs when it cannot land completely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Atomicity {
    /// The keys land one by one, in key order. A failure can leave a prefix
    /// of the keys landed.
    PerKey,
    /// Every key lands, or no key lands.
    WholeSet,
}

/// One change to canonical state: the keys to put and the keys to delete.
#[derive(Clone, PartialEq, Eq)]
pub struct WriteSet {
    base: Option<Arc<StateView>>,
    puts: BTreeMap<StateKey, Vec<u8>>,
    deletes: BTreeSet<StateKey>,
    atomicity: Atomicity,
}

/// Shows the base as its number of keys. The bytes of the base are not
/// shown, and the base is not hashed.
impl std::fmt::Debug for WriteSet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct BaseKeys(usize);
        impl std::fmt::Debug for BaseKeys {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "<{} keys>", self.0)
            }
        }
        let base = self
            .base
            .as_ref()
            .map(|base| BaseKeys(base.entries().len()));
        formatter
            .debug_struct("WriteSet")
            .field("base", &base)
            .field("puts", &self.puts)
            .field("deletes", &self.deletes)
            .field("atomicity", &self.atomicity)
            .finish()
    }
}

impl WriteSet {
    pub const fn new(atomicity: Atomicity) -> Self {
        Self {
            base: None,
            puts: BTreeMap::new(),
            deletes: BTreeSet::new(),
            atomicity,
        }
    }

    /// Refuses the set when the stored keys and bytes are no longer those
    /// of `base`.
    #[must_use]
    pub fn on_base(mut self, base: Arc<StateView>) -> Self {
        self.base = Some(base);
        self
    }

    #[must_use]
    pub fn put(mut self, key: StateKey, bytes: Vec<u8>) -> Self {
        self.puts.insert(key, bytes);
        self
    }

    #[must_use]
    pub fn delete(mut self, key: StateKey) -> Self {
        self.deletes.insert(key);
        self
    }

    /// The revision of the base, computed once for each view.
    pub fn base(&self) -> Option<&Revision> {
        self.base.as_deref().map(StateView::revision)
    }

    /// The keys and bytes of the base.
    pub fn base_view(&self) -> Option<&StateView> {
        self.base.as_deref()
    }

    /// The same puts, deletes and atomicity with no base.
    pub(crate) fn without_base(&self) -> Self {
        Self {
            base: None,
            puts: self.puts.clone(),
            deletes: self.deletes.clone(),
            atomicity: self.atomicity,
        }
    }

    /// The same set with the atomicity of each key, on the same base, if it
    /// has one key; else the set as it is.
    pub(crate) fn narrowed(self) -> Self {
        if self.key_count() == 1 {
            Self {
                atomicity: Atomicity::PerKey,
                ..self
            }
        } else {
            self
        }
    }

    /// Refuses the set with [`PublishFailure::BaseMoved`] when `current` is
    /// not the base of the set. Only a moved base is hashed.
    pub(crate) fn ensure_base(
        &self,
        current: &BTreeMap<StateKey, Vec<u8>>,
    ) -> Result<(), PublishFailure> {
        match self.base_view() {
            Some(base) if base.entries() != current => Err(PublishFailure::BaseMoved {
                base: base.revision().clone(),
                current: Revision::of(current),
            }),
            _ => Ok(()),
        }
    }

    pub const fn puts(&self) -> &BTreeMap<StateKey, Vec<u8>> {
        &self.puts
    }

    pub const fn deletes(&self) -> &BTreeSet<StateKey> {
        &self.deletes
    }

    pub const fn atomicity(&self) -> Atomicity {
        self.atomicity
    }

    /// The number of keys that the set changes.
    pub fn key_count(&self) -> usize {
        self.puts.len() + self.deletes.len()
    }

    /// Refuses a set that puts and deletes the same key.
    pub fn ensure_disjoint(&self) -> Result<(), PublishFailure> {
        let both = self.deletes.iter().find(|key| self.puts.contains_key(*key));
        both.map_or(Ok(()), |key| {
            Err(PublishFailure::Refused(crate::refusal::fault!(
                "the write set puts and deletes `{}`",
                key.as_str()
            )))
        })
    }

    /// The tree after this set lands on `entries`.
    pub fn apply_to(&self, entries: &mut BTreeMap<StateKey, Vec<u8>>) {
        for key in &self.deletes {
            entries.remove(key);
        }
        for (key, bytes) in &self.puts {
            entries.insert(key.clone(), bytes.clone());
        }
    }
}

/// Why a write set did not land completely.
#[derive(Debug, thiserror::Error)]
pub enum PublishFailure {
    /// No key landed. A per-key set that fails before its first key
    /// changes is refused.
    #[error("{0:#}")]
    Refused(anyhow::Error),
    /// The stored keys and bytes were not those of the base of the set. No
    /// key landed.
    #[error("the state moved from revision {} to {} before the write", .base.as_str(), .current.as_str())]
    BaseMoved { base: Revision, current: Revision },
    /// The failure came after a stored key changed, or after a whole set
    /// began to land. A per-key set can have a prefix of its keys landed.
    #[error("{0:#}")]
    Started(anyhow::Error),
}

/// Where canonical state lives.
///
/// An adapter is synchronous. It returns every key with the bytes it was
/// given. The engine calls it in the section that serializes access to the
/// state; see each adapter for what that section is. Publishes are
/// serialized. A set with one key lands completely or not at all. A set
/// whose stored keys and bytes differ from its base is refused before any
/// key lands. A failure after a stored key changed, or after a whole set
/// began to land, is [`PublishFailure::Started`]. A set with no key writes no data: the
/// adapter checks its base, then returns.
pub trait StateStorage: Send + Sync + std::fmt::Debug {
    /// One consistent view of every key and its bytes.
    fn read_view(&self) -> Result<StateView, StorageError>;

    /// Lands `writes`. An adapter honours the atomicity of the set or
    /// refuses the set before any key lands.
    ///
    /// A copy of a write set holds the base view. When an adapter keeps a
    /// copy after `publish` returns, the section copies the whole tree on
    /// its next write.
    fn publish(&self, writes: WriteSet) -> Result<(), PublishFailure>;
}

/// The section that serializes the engine's access to one repository's
/// canonical state.
///
/// The engine holds the section while it reads a view and publishes the
/// write sets decided on that view. A second acquire waits until the first
/// section ends. Each adapter names the lock that it needs: the local-disk
/// adapter a lock file and recovery, the memory adapter a lock in the
/// process.
pub trait SectionLock: Send + Sync + std::fmt::Debug {
    /// Holds the section until the returned value drops.
    fn acquire(&self) -> anyhow::Result<Box<dyn Send>>;
}

/// Where an adapter that holds canonical state in memory keeps each write
/// set, such as a database of the host.
///
/// The persistence is synchronous, as the port is.
pub trait StatePersistence: Send + Sync + std::fmt::Debug {
    /// Keeps `writes` before the adapter lands them. An error refuses the
    /// set: the adapter returns the error and changes no key.
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure>;
}

/// Why a storage call did not complete.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("invalid state key `{key}`: {reason}")]
    InvalidKey { key: String, reason: &'static str },
    #[error("{0:#}")]
    Read(anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::{Atomicity, StateKey, StateView, WriteSet};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn key(key: &str) -> StateKey {
        StateKey::new(key).unwrap()
    }

    #[test]
    fn debug_shows_the_base_as_its_number_of_keys() {
        let base = BTreeMap::from([(key("a"), b"secret".to_vec()), (key("b"), b"x".to_vec())]);
        let writes = WriteSet::new(Atomicity::PerKey)
            .put(key("c"), vec![1])
            .delete(key("d"));
        let based = writes.clone().on_base(Arc::new(StateView::new(base)));
        assert_eq!(
            format!("{based:?}"),
            "WriteSet { base: Some(<2 keys>), puts: {StateKey(\"c\"): [1]}, \
             deletes: {StateKey(\"d\")}, atomicity: PerKey }"
        );
        assert_eq!(
            format!("{writes:?}"),
            "WriteSet { base: None, puts: {StateKey(\"c\"): [1]}, \
             deletes: {StateKey(\"d\")}, atomicity: PerKey }"
        );
    }
}
