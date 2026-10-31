//! The storage port: where canonical state lives, apart from the engine.
//!
//! The engine reads canonical state as one consistent view. A view holds the
//! manifest and every shard under `.provenance/state`, byte for byte, keyed
//! by the path below the state directory with `/` between its parts. A
//! revision digests every key and its bytes. Derived state, such as the
//! verification runs under `.provenance/cache`, is not canonical state and is
//! not in a view.
//!
//! The engine publishes a change as one write set. A write set states the
//! atomicity that the change needs, and may carry the revision that the
//! change was decided on.
//!
//! The port is synchronous: the engine runs where its storage is.

#[cfg(any(test, feature = "test-fixture"))]
pub mod contract;
pub mod local;
#[cfg(any(test, feature = "test-fixture"))]
pub mod memory;

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateView {
    revision: Revision,
    entries: BTreeMap<StateKey, Vec<u8>>,
}

impl StateView {
    pub fn new(entries: BTreeMap<StateKey, Vec<u8>>) -> Self {
        Self {
            revision: Revision::of(&entries),
            entries,
        }
    }

    pub const fn revision(&self) -> &Revision {
        &self.revision
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteSet {
    base: Option<Revision>,
    puts: BTreeMap<StateKey, Vec<u8>>,
    deletes: BTreeSet<StateKey>,
    atomicity: Atomicity,
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

    /// Refuses the set when the state is no longer at `base`.
    #[must_use]
    pub fn on_base(mut self, base: Revision) -> Self {
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

    pub const fn base(&self) -> Option<&Revision> {
        self.base.as_ref()
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
            Err(PublishFailure::Refused(anyhow::anyhow!(
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
    /// No key landed.
    #[error("{0:#}")]
    Refused(anyhow::Error),
    /// The state was not at the base revision of the set. No key landed.
    #[error("the state moved from revision {} to {} before the write", .base.as_str(), .current.as_str())]
    BaseMoved { base: Revision, current: Revision },
    /// The failure came after the first key began to land. A per-key set
    /// can have a prefix of its keys landed.
    #[error("{0:#}")]
    Started(anyhow::Error),
}

/// Where canonical state lives.
///
/// An adapter is synchronous. It returns every key with the bytes it was
/// given. The engine calls it in the section that serializes access to the
/// state; see each adapter for what that section is. Publishes are
/// serialized. A set with one key lands completely or not at all. A set
/// whose base revision moved is refused before any key lands.
pub trait StateStorage: Send + Sync + std::fmt::Debug {
    /// One consistent view of every key and its bytes.
    fn read_view(&self) -> Result<StateView, StorageError>;

    /// Lands `writes` and returns the revision of the whole tree after the
    /// write. An adapter honours the atomicity of the set or refuses the set
    /// before any key lands.
    fn publish(&self, writes: WriteSet) -> Result<Revision, PublishFailure>;
}

/// Why a storage call did not complete.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("invalid state key `{key}`: {reason}")]
    InvalidKey { key: String, reason: &'static str },
    #[error("{0:#}")]
    Read(anyhow::Error),
}
