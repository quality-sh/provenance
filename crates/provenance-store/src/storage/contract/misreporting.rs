//! A store whose answer does not match its state, and the case that refuses
//! it. These tests are an implementation aid with no Rule binding: they show
//! that the case for a failed per-key set checks the answer against the
//! state, so a store that accepts it is not just loosely checked.

use super::whole_landing::WholeLanding;
use super::*;

/// How the store gets its answer wrong.
#[derive(Debug)]
enum Misreport {
    /// Lands the set whole, so no key lands, and reports that landing
    /// started.
    StartedWithNoKey,
    /// Lands the keys before the failure and refuses the set.
    RefusedAfterPrefix,
    /// Lands each put as its own set, so the keys on both sides of a broken
    /// key land, and reports that landing started.
    SkipsBrokenKey,
}

#[derive(Debug)]
struct Misreporting {
    whole: WholeLanding,
    misreport: Misreport,
}

impl Misreporting {
    fn new(misreport: Misreport) -> Self {
        Self {
            whole: WholeLanding::default(),
            misreport,
        }
    }
}

impl StateStorage for Misreporting {
    fn read_view(&self) -> Result<StateView, crate::storage::StorageError> {
        self.whole.read_view()
    }

    fn publish(&self, writes: WriteSet) -> Result<(), PublishFailure> {
        match self.misreport {
            Misreport::StartedWithNoKey => match self.whole.publish(writes) {
                Err(PublishFailure::Refused(error)) => Err(PublishFailure::Started(error)),
                answer => answer,
            },
            Misreport::RefusedAfterPrefix => match self.whole.storage.publish(writes) {
                Err(PublishFailure::Started(error)) => Err(PublishFailure::Refused(error)),
                answer => answer,
            },
            Misreport::SkipsBrokenKey => {
                let mut failure = None;
                for (key, bytes) in writes.puts() {
                    let one = WriteSet::new(Atomicity::PerKey).put(key.clone(), bytes.clone());
                    if let Err(error) = self.whole.storage.publish(one) {
                        failure.get_or_insert(error);
                    }
                }
                failure.map_or(Ok(()), |error| {
                    Err(PublishFailure::Started(crate::refusal::fault!("{error}")))
                })
            }
        }
    }
}

impl Harness for Misreporting {
    fn seed(&mut self, entries: &[(&str, &[u8])]) {
        self.whole.seed(entries);
    }

    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R {
        call(self)
    }

    fn break_key(&mut self, key: &str) {
        self.whole.break_key(key);
    }

    fn repair_key(&mut self, key: &str) {
        self.whole.repair_key(key);
    }
}

#[test]
#[should_panic(expected = "a started set lands the keys before the failure")]
fn the_case_refuses_a_store_that_lands_no_key_and_reports_started() {
    a_failed_per_key_set_leaves_a_landed_prefix(Misreporting::new(Misreport::StartedWithNoKey));
}

#[test]
#[should_panic(expected = "a refused set lands no key")]
fn the_case_refuses_a_store_that_lands_a_prefix_and_refuses_the_set() {
    a_failed_per_key_set_leaves_a_landed_prefix(Misreporting::new(Misreport::RefusedAfterPrefix));
}

#[test]
#[should_panic(expected = "a started set lands the keys before the failure")]
fn the_case_refuses_a_store_that_lands_keys_after_the_broken_key() {
    a_failed_per_key_set_leaves_a_landed_prefix(Misreporting::new(Misreport::SkipsBrokenKey));
}
