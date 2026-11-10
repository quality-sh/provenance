//! The storage contract suite of `provenance-store` on the simulated
//! storage, with no fault planned. So a difference between the simulated
//! storage and the real adapters fails here, and not as a false law break.
//! The nightly engine job runs these tests; CI does not run them.

use provenance_simulation::storage::{SimulatedStorage, World};
use provenance_store::storage::contract::{self, Harness};
use provenance_store::storage::{StateKey, StateStorage};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The simulated storage, used outside every engine, so each port call runs
/// at once.
struct Simulated {
    world: Arc<World>,
}

impl Default for Simulated {
    fn default() -> Self {
        Self {
            world: World::new(BTreeMap::new(), None),
        }
    }
}

impl Harness for Simulated {
    fn seed(&mut self, entries: &[(&str, &[u8])]) {
        let mut state = self.world.entries();
        for (key, bytes) in entries {
            state.insert(StateKey::new(*key).unwrap(), bytes.to_vec());
        }
        self.world = World::new(state, None);
    }

    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R {
        call(&SimulatedStorage(self.world.clone()))
    }

    fn break_key(&mut self, key: &str) {
        self.world.break_key(StateKey::new(key).unwrap());
    }
}

// No Rule: the store's tests verify the contract on the real adapters; this
// test checks that the simulated storage keeps the same contract.
#[test]
#[ignore = "a check of the nightly simulation"]
fn the_simulated_storage_keeps_the_storage_contract() {
    contract::view_returns_every_key_with_its_bytes(Simulated::default());
    contract::empty_state_reads_as_an_empty_view(&Simulated::default());
    contract::revision_follows_every_key_and_byte(Simulated::default());
    contract::publish_lands_puts_and_deletes(Simulated::default());
    contract::publish_on_the_current_base_lands(Simulated::default());
    contract::publish_on_a_moved_base_is_refused(Simulated::default());
    contract::publish_on_a_base_with_changed_bytes_is_refused(Simulated::default());
    contract::a_whole_set_on_a_moved_base_is_refused(Simulated::default());
    contract::publish_refuses_a_put_and_delete_of_one_key(Simulated::default());
    contract::a_one_key_set_lands_all_of_its_bytes(Simulated::default());
    contract::a_failed_one_key_set_lands_nothing(Simulated::default());
    contract::a_failed_per_key_set_leaves_a_landed_prefix(Simulated::default());
    contract::a_two_key_whole_set_lands_both_keys(Simulated::default());
    contract::a_failed_whole_set_lands_no_key(Simulated::default());
}
