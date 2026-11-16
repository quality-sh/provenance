//! The stored keys of the memory adapter.

use crate::storage::StateKey;
use std::collections::{BTreeMap, BTreeSet};

/// The keys and bytes that the memory adapter holds. A key changes only
/// through [`Entries::write`], so a broken key fails each write of it,
/// whichever set makes the write.
#[derive(Clone, Debug, Default)]
pub(super) struct Entries {
    map: BTreeMap<StateKey, Vec<u8>>,
    broken: BTreeSet<StateKey>,
}

impl Entries {
    pub(super) const fn new(map: BTreeMap<StateKey, Vec<u8>>) -> Self {
        Self {
            map,
            broken: BTreeSet::new(),
        }
    }

    pub(super) const fn map(&self) -> &BTreeMap<StateKey, Vec<u8>> {
        &self.map
    }

    /// Puts `bytes` at `key`, or removes `key` when `bytes` is `None`. The
    /// write of a broken key fails and changes nothing.
    pub(super) fn write(&mut self, key: &StateKey, bytes: Option<&Vec<u8>>) -> anyhow::Result<()> {
        if self.broken.contains(key) {
            return Err(crate::refusal::fault!(
                "the key `{}` failed to land",
                key.as_str()
            ));
        }
        match bytes {
            Some(bytes) => self.map.insert(key.clone(), bytes.clone()),
            None => self.map.remove(key),
        };
        Ok(())
    }

    /// Makes each later write of `key` fail.
    #[cfg(test)]
    pub(super) fn break_key(&mut self, key: StateKey) {
        self.broken.insert(key);
    }
}
