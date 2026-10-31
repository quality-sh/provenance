//! The in-memory adapter, for tests: canonical state as a map of keys to bytes.

use super::{StateKey, StateStorage, StateView, StorageError};
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

/// Canonical state in memory. One mutex serializes every call.
#[derive(Debug, Default)]
pub struct MemoryStateStorage {
    entries: Mutex<BTreeMap<StateKey, Vec<u8>>>,
}

impl MemoryStateStorage {
    pub fn new(entries: BTreeMap<StateKey, Vec<u8>>) -> Self {
        Self {
            entries: Mutex::new(entries),
        }
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, BTreeMap<StateKey, Vec<u8>>> {
        // Every change replaces whole values, so a poisoned map is intact.
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl StateStorage for MemoryStateStorage {
    fn read_view(&self) -> Result<StateView, StorageError> {
        Ok(StateView::new(self.entries().clone()))
    }
}
