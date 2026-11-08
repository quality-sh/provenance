//! The in-memory adapter: canonical state as a map of keys to bytes.
//!
//! A host that loads a project into memory, such as a Worker, gives the
//! adapter a persistence that keeps each write set. Tests use the adapter
//! with no persistence.

use super::{
    Atomicity, PublishFailure, Revision, SectionLock, StateKey, StatePersistence, StateStorage,
    StateView, StorageError, WriteSet,
};
use provenance_macros::rule;
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// Canonical state in memory. One mutex serializes every call. A set with no
/// failing key lands whole, which satisfies a per-key set too.
#[derive(Debug, Default)]
pub struct MemoryStateStorage {
    state: Mutex<State>,
    persistence: Option<Arc<dyn StatePersistence>>,
}

#[derive(Debug, Default)]
struct State {
    entries: BTreeMap<StateKey, Vec<u8>>,
    /// A key that fails to land, so the contract suite can see a failure
    /// partway through a set.
    failing: Option<StateKey>,
}

impl MemoryStateStorage {
    pub const fn new(entries: BTreeMap<StateKey, Vec<u8>>) -> Self {
        Self {
            state: Mutex::new(State {
                entries,
                failing: None,
            }),
            persistence: None,
        }
    }

    /// Canonical state loaded from `view`. Each publish gives its write set
    /// to `persistence` before the set changes the memory.
    pub fn loaded(view: StateView, persistence: Arc<dyn StatePersistence>) -> Self {
        Self {
            persistence: Some(persistence),
            ..Self::new(view.into_entries())
        }
    }

    /// Makes every later landing of `key` fail. The failure comes before the
    /// persistence sees the set.
    #[cfg(test)]
    pub(crate) fn fail_on(&self, key: StateKey) {
        self.state().failing = Some(key);
    }

    /// Gives `writes` to the persistence, if there is one.
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
        self.persistence
            .as_ref()
            .map_or(Ok(()), |persistence| persistence.persist(writes))
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
    /// landed. The persistence keeps the keys that land before any of them
    /// lands: the whole set, or the prefix of a per-key set up to a failing
    /// key. When it refuses them, no key changes.
    #[rule("rule_memory_storage_persists_each_publish")]
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
                        self.persist(&prefix_before(&writes, key))?;
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
        self.persist(&writes)?;
        state.entries = landed;
        Ok(Revision::of(&state.entries))
    }
}

/// The section of a repository whose state is in memory: a lock in the
/// process. A clone shares the lock.
#[derive(Clone, Debug, Default)]
pub struct MemorySectionLock {
    held: Arc<(Mutex<bool>, Condvar)>,
}

impl SectionLock for MemorySectionLock {
    fn acquire(&self) -> anyhow::Result<Box<dyn Send>> {
        let (held, released) = &*self.held;
        let mut held = released
            .wait_while(
                held.lock().unwrap_or_else(PoisonError::into_inner),
                |held| *held,
            )
            .unwrap_or_else(PoisonError::into_inner);
        *held = true;
        drop(held);
        Ok(Box::new(MemorySection(Arc::clone(&self.held))))
    }
}

/// A held memory section. Released on drop.
struct MemorySection(Arc<(Mutex<bool>, Condvar)>);

impl Drop for MemorySection {
    fn drop(&mut self) {
        let (held, released) = &*self.0;
        *held.lock().unwrap_or_else(PoisonError::into_inner) = false;
        released.notify_one();
    }
}

/// The keys of a per-key `writes` that land before `failing`, in key order.
fn prefix_before(writes: &WriteSet, failing: &StateKey) -> WriteSet {
    let mut prefix = WriteSet::new(Atomicity::PerKey);
    for key in writes.deletes().range(..failing) {
        prefix = prefix.delete(key.clone());
    }
    for (key, bytes) in writes.puts().range(..failing) {
        prefix = prefix.put(key.clone(), bytes.clone());
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_macros::verifies;

    #[derive(Debug, Default)]
    struct Kept(Mutex<Vec<WriteSet>>);

    impl StatePersistence for Kept {
        fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
            self.0.lock().unwrap().push(writes.clone());
            Ok(())
        }
    }

    fn key(key: &str) -> StateKey {
        StateKey::new(key).unwrap()
    }

    /// Memory that holds `old` at `b`, keeps each set in the returned
    /// persistence, and fails to land `b`.
    fn failing_on_b() -> (MemoryStateStorage, Arc<Kept>) {
        let kept = Arc::new(Kept::default());
        let view = StateView::new(BTreeMap::from([(key("b"), b"old".to_vec())]));
        let storage = MemoryStateStorage::loaded(view, kept.clone());
        storage.fail_on(key("b"));
        (storage, kept)
    }

    fn put_a_b_c(atomicity: Atomicity) -> WriteSet {
        WriteSet::new(atomicity)
            .put(key("a"), b"a".to_vec())
            .put(key("b"), b"b".to_vec())
            .put(key("c"), b"c".to_vec())
    }

    #[test]
    #[verifies("rule_memory_storage_persists_each_publish", examples)]
    fn a_refused_whole_set_reaches_no_persistence() {
        let (storage, kept) = failing_on_b();
        let before = storage.read_view().unwrap();
        let result = storage.publish(put_a_b_c(Atomicity::WholeSet));
        assert!(matches!(result, Err(PublishFailure::Refused(_))));
        assert!(kept.0.lock().unwrap().is_empty());
        assert_eq!(storage.read_view().unwrap(), before);
    }

    #[test]
    #[verifies("rule_memory_storage_persists_each_publish", examples)]
    fn the_persistence_keeps_only_the_landed_prefix_of_a_per_key_set() {
        let (storage, kept) = failing_on_b();
        let result = storage.publish(put_a_b_c(Atomicity::PerKey));
        assert!(matches!(result, Err(PublishFailure::Started(_))));
        let prefix = WriteSet::new(Atomicity::PerKey).put(key("a"), b"a".to_vec());
        assert_eq!(*kept.0.lock().unwrap(), [prefix]);
        let landed = BTreeMap::from([(key("a"), b"a".to_vec()), (key("b"), b"old".to_vec())]);
        assert_eq!(storage.read_view().unwrap().into_entries(), landed);
    }
}
