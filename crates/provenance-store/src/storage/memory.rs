//! The in-memory adapter, for tests: canonical state as a map of keys to bytes.

use super::{
    Atomicity, PublishFailure, Revision, StateKey, StateStorage, StateView, StorageError, WriteSet,
};
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

/// Canonical state in memory. One mutex serializes every call. A set with no
/// failing key lands whole, which satisfies a per-key set too.
#[derive(Debug, Default)]
pub struct MemoryStateStorage {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    entries: BTreeMap<StateKey, Vec<u8>>,
    /// A key that fails to land, so the contract suite can see a failure
    /// partway through a set.
    failing: Option<StateKey>,
}

impl MemoryStateStorage {
    pub fn new(entries: BTreeMap<StateKey, Vec<u8>>) -> Self {
        Self {
            state: Mutex::new(State {
                entries,
                failing: None,
            }),
        }
    }

    /// Makes every later landing of `key` fail.
    pub fn fail_on(&self, key: StateKey) {
        self.state().failing = Some(key);
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        // Every change replaces whole values, so a poisoned state is intact.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl StateStorage for MemoryStateStorage {
    fn read_view(&self) -> Result<StateView, StorageError> {
        Ok(StateView::new(self.state().entries.clone()))
    }

    /// A per-key set lands its keys in key order up to a failing key; a
    /// whole set lands on a copy that replaces the state only when every key
    /// landed.
    fn publish(&self, writes: WriteSet) -> Result<Revision, PublishFailure> {
        writes.ensure_disjoint()?;
        let mut state = self.state();
        if let Some(base) = writes.base() {
            let current = Revision::of(&state.entries);
            if &current != base {
                return Err(PublishFailure::BaseMoved {
                    base: base.clone(),
                    current,
                });
            }
        }
        let failing = state.failing.clone();
        let mut landed = state.entries.clone();
        let mut changes: BTreeMap<&StateKey, Option<&Vec<u8>>> = BTreeMap::new();
        changes.extend(writes.deletes().iter().map(|key| (key, None)));
        changes.extend(writes.puts().iter().map(|(key, bytes)| (key, Some(bytes))));
        for (key, bytes) in changes {
            if failing.as_ref() == Some(key) {
                let error = anyhow::anyhow!("the key `{}` failed to land", key.as_str());
                return Err(match writes.atomicity() {
                    Atomicity::WholeSet => PublishFailure::Refused(error),
                    Atomicity::PerKey => {
                        state.entries = landed;
                        PublishFailure::Started(error)
                    }
                });
            }
            match bytes {
                Some(bytes) => landed.insert(key.clone(), bytes.clone()),
                None => landed.remove(key),
            };
        }
        state.entries = landed;
        Ok(Revision::of(&state.entries))
    }
}
