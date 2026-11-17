//! The memory adapter given each set as a whole set. It models a store that
//! lands a per-key set whole, so a per-key set that fails lands no key.

use super::*;
use crate::storage::memory::MemoryStateStorage;

#[derive(Debug, Default)]
pub(super) struct WholeLanding {
    pub(super) storage: MemoryStateStorage,
}

impl StateStorage for WholeLanding {
    fn read_view(&self) -> Result<StateView, crate::storage::StorageError> {
        self.storage.read_view()
    }

    /// Gives `writes` to the memory as a whole set, with its base and keys.
    fn publish(&self, writes: WriteSet) -> Result<(), PublishFailure> {
        self.storage.publish(WriteSet {
            atomicity: Atomicity::WholeSet,
            ..writes
        })
    }
}

impl Harness for WholeLanding {
    fn seed(&mut self, entries: &[(&str, &[u8])]) {
        let mut state = self.storage.read_view().unwrap().into_entries();
        state.extend(expected(entries));
        self.storage = MemoryStateStorage::new(state);
    }

    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R {
        call(self)
    }

    fn break_key(&mut self, key: &str) {
        self.storage.fail_on(StateKey::new(key).unwrap());
    }
}
