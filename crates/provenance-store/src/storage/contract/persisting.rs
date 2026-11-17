//! The memory adapter with a persistence that keeps each write set and can
//! refuse one.

use super::*;
use crate::storage::memory::MemoryStateStorage;
use crate::storage::StatePersistence;
use provenance_macros::verifies;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct Kept {
    sets: Mutex<Vec<WriteSet>>,
    refuse: AtomicBool,
    /// Keys that the persistence refuses to keep, in each set that names
    /// them.
    broken: Mutex<BTreeSet<StateKey>>,
}

impl StatePersistence for Kept {
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
        if self.refuse.load(SeqCst) {
            return Err(PublishFailure::Refused(crate::refusal::fault!("refused")));
        }
        let broken = self.broken.lock().unwrap();
        let mut keys = writes.deletes().iter().chain(writes.puts().keys());
        if let Some(key) = keys.find(|key| broken.contains(*key)) {
            return Err(PublishFailure::Refused(crate::refusal::fault!(
                "the persistence failed to keep `{}`",
                key.as_str()
            )));
        }
        drop(broken);
        self.sets.lock().unwrap().push(writes.clone());
        Ok(())
    }
}

#[derive(Default)]
pub struct Persisting {
    kept: Arc<Kept>,
    storage: MemoryStateStorage,
}

impl Harness for Persisting {
    fn seed(&mut self, entries: &[(&str, &[u8])]) {
        let mut state = self.storage.read_view().unwrap().into_entries();
        state.extend(expected(entries));
        self.storage = MemoryStateStorage::loaded(StateView::new(state), self.kept.clone());
    }

    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R {
        call(&self.storage)
    }

    /// The memory and the persistence both fail each later write of `key`.
    fn break_key(&mut self, key: &str) {
        let key = StateKey::new(key).unwrap();
        self.kept.broken.lock().unwrap().insert(key.clone());
        self.storage.fail_on(key);
    }
}

#[test]
#[verifies("rule_memory_storage_persists_each_publish", examples)]
fn the_persistence_keeps_each_published_set() {
    let mut harness = Persisting::default();
    harness.seed(&[("manifest.json", b"{}")]);
    let writes = WriteSet::new(Atomicity::PerKey).put(key(A), b"a".to_vec());
    publish(&harness, writes.clone()).unwrap();
    assert_eq!(*harness.kept.sets.lock().unwrap(), [writes]);
}

#[test]
#[verifies("rule_memory_storage_persists_each_publish", examples)]
fn a_refused_persist_changes_no_key() {
    let mut harness = Persisting::default();
    harness.seed(&[("manifest.json", b"{}"), (A, b"old")]);
    harness
        .kept
        .refuse
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let before = read(&harness);
    let writes = WriteSet::new(Atomicity::WholeSet)
        .put(key(A), b"a".to_vec())
        .put(key(B), b"b".to_vec())
        .delete(key("manifest.json"));
    assert!(matches!(
        publish(&harness, writes),
        Err(PublishFailure::Refused(_))
    ));
    assert_eq!(read(&harness), before);
    assert!(harness.kept.sets.lock().unwrap().is_empty());
}
