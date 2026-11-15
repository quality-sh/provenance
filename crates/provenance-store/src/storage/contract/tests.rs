//! The contract cases on the local adapter and on the memory adapter, with
//! and without a persistence.

use super::local_harness::local;
use super::persisting::Persisting;
use super::*;
use crate::storage::memory::MemoryStateStorage;
use provenance_macros::verifies;

#[derive(Default)]
struct Memory {
    storage: MemoryStateStorage,
}

impl Harness for Memory {
    fn seed(&mut self, entries: &[(&str, &[u8])]) {
        let mut state = self.storage.read_view().unwrap().entries().clone();
        state.extend(expected(entries));
        self.storage = MemoryStateStorage::new(state);
    }

    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R {
        call(&self.storage)
    }

    fn break_key(&mut self, key: &str) {
        self.storage.fail_on(StateKey::new(key).unwrap());
    }
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn each_adapter_publish_lands_puts_and_deletes() {
    publish_lands_puts_and_deletes(local());
    publish_lands_puts_and_deletes(Memory::default());
    publish_lands_puts_and_deletes(Persisting::default());
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn each_adapter_publish_on_the_current_base_lands() {
    publish_on_the_current_base_lands(local());
    publish_on_the_current_base_lands(Memory::default());
    publish_on_the_current_base_lands(Persisting::default());
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
#[verifies("rule_adapter_refuses_a_set_on_a_moved_base", examples)]
fn each_adapter_publish_on_a_moved_base_is_refused() {
    publish_on_a_moved_base_is_refused(local());
    publish_on_a_moved_base_is_refused(Memory::default());
    publish_on_a_moved_base_is_refused(Persisting::default());
}

#[test]
#[verifies("rule_adapter_refuses_a_set_on_a_moved_base", examples)]
fn each_adapter_publish_on_a_base_with_changed_bytes_is_refused() {
    publish_on_a_base_with_changed_bytes_is_refused(local());
    publish_on_a_base_with_changed_bytes_is_refused(Memory::default());
    publish_on_a_base_with_changed_bytes_is_refused(Persisting::default());
}

#[test]
#[verifies("rule_adapter_refuses_a_set_on_a_moved_base", examples)]
fn each_adapter_a_whole_set_on_a_moved_base_is_refused() {
    a_whole_set_on_a_moved_base_is_refused(local());
    a_whole_set_on_a_moved_base_is_refused(Memory::default());
    a_whole_set_on_a_moved_base_is_refused(Persisting::default());
}

/// The local adapter makes no transaction and no marker for it.
#[test]
#[verifies("rule_empty_write_set_writes_no_data", examples)]
fn each_adapter_an_empty_set_changes_nothing() {
    let local = an_empty_set_changes_nothing(local());
    let transactions = local.layout.import_transactions_dir();
    assert_eq!(std::fs::read_dir(transactions).unwrap().count(), 0);
    assert!(!local.layout.publication_marker_path().exists());
    an_empty_set_changes_nothing(Memory::default());
    an_empty_set_changes_nothing(Persisting::default());
}

#[test]
#[verifies("rule_empty_write_set_writes_no_data", examples)]
fn each_adapter_an_empty_set_lands_while_each_stored_key_is_broken() {
    an_empty_set_lands_while_each_stored_key_is_broken(local());
    an_empty_set_lands_while_each_stored_key_is_broken(Memory::default());
    an_empty_set_lands_while_each_stored_key_is_broken(Persisting::default());
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
#[verifies("rule_one_key_write_set_lands_whole", examples)]
fn each_adapter_a_one_key_set_lands_while_each_other_key_is_broken() {
    a_one_key_set_lands_while_each_other_key_is_broken(local());
    a_one_key_set_lands_while_each_other_key_is_broken(Memory::default());
    a_one_key_set_lands_while_each_other_key_is_broken(Persisting::default());
}

#[test]
#[verifies("rule_adapter_refuses_a_set_on_a_moved_base", examples)]
fn each_adapter_an_empty_set_on_a_moved_base_is_refused() {
    an_empty_set_on_a_moved_base_is_refused(local());
    an_empty_set_on_a_moved_base_is_refused(Memory::default());
    an_empty_set_on_a_moved_base_is_refused(Persisting::default());
}

#[test]
#[verifies("rule_one_key_write_set_lands_whole", examples)]
fn each_adapter_a_one_key_set_lands_all_of_its_bytes() {
    a_one_key_set_lands_all_of_its_bytes(local());
    a_one_key_set_lands_all_of_its_bytes(Memory::default());
    a_one_key_set_lands_all_of_its_bytes(Persisting::default());
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn each_adapter_publish_refuses_a_put_and_delete_of_one_key() {
    publish_refuses_a_put_and_delete_of_one_key(local());
    publish_refuses_a_put_and_delete_of_one_key(Memory::default());
    publish_refuses_a_put_and_delete_of_one_key(Persisting::default());
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
#[verifies("rule_one_key_write_set_lands_whole", examples)]
#[verifies("rule_failure_before_a_key_changes_is_refused", examples)]
fn each_adapter_a_failed_one_key_set_lands_nothing() {
    a_failed_one_key_set_lands_nothing(local());
    a_failed_one_key_set_lands_nothing(Memory::default());
    a_failed_one_key_set_lands_nothing(Persisting::default());
}

#[test]
#[verifies("rule_failure_before_a_key_changes_is_refused", examples)]
fn each_adapter_a_per_key_set_that_fails_at_its_first_key_is_refused() {
    a_per_key_set_that_fails_at_its_first_key_is_refused(local());
    a_per_key_set_that_fails_at_its_first_key_is_refused(Memory::default());
    a_per_key_set_that_fails_at_its_first_key_is_refused(Persisting::default());
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
#[verifies("rule_failure_after_landing_starts_is_an_unknown_outcome", examples)]
fn each_adapter_a_failed_per_key_set_leaves_a_landed_prefix() {
    a_failed_per_key_set_leaves_a_landed_prefix(local());
    a_failed_per_key_set_leaves_a_landed_prefix(Memory::default());
    a_failed_per_key_set_leaves_a_landed_prefix(Persisting::default());
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
#[verifies("rule_stage_lands_as_one_whole_set", examples)]
fn each_adapter_lands_a_two_key_whole_set_and_rolls_back_a_failed_one() {
    a_two_key_whole_set_lands_both_keys(local());
    a_failed_whole_set_lands_no_key(local());
    a_two_key_whole_set_lands_both_keys(Memory::default());
    a_failed_whole_set_lands_no_key(Memory::default());
    a_two_key_whole_set_lands_both_keys(Persisting::default());
    a_failed_whole_set_lands_no_key(Persisting::default());
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn each_adapter_view_returns_every_key_with_its_bytes() {
    view_returns_every_key_with_its_bytes(local());
    view_returns_every_key_with_its_bytes(Memory::default());
    view_returns_every_key_with_its_bytes(Persisting::default());
}

#[test]
fn each_adapter_reads_an_empty_state_as_an_empty_view() {
    empty_state_reads_as_an_empty_view(&local());
    empty_state_reads_as_an_empty_view(&Memory::default());
    empty_state_reads_as_an_empty_view(&Persisting::default());
}

#[test]
fn each_adapter_revision_follows_every_key_and_byte() {
    revision_follows_every_key_and_byte(local());
    revision_follows_every_key_and_byte(Memory::default());
    revision_follows_every_key_and_byte(Persisting::default());
}

/// A symbolic link in the state directory is not a state file.
#[cfg(unix)]
#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn local_view_leaves_out_a_symbolic_link() {
    let mut harness = local();
    harness.seed(&[("manifest.json", b"{}")]);
    let outside = harness.layout.root().join("outside.txt");
    std::fs::write(&outside, b"outside").unwrap();
    std::os::unix::fs::symlink(&outside, harness.layout.state_dir().join("link")).unwrap();
    let view = read(&harness);
    assert_eq!(view.entries(), &expected(&[("manifest.json", b"{}")]));
}

/// Verification runs under `.provenance/cache` are derived state.
#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn local_view_leaves_out_derived_state() {
    let mut harness = local();
    harness.seed(&[("manifest.json", b"{}")]);
    let runs = harness
        .layout
        .verification_runs_path(&provenance_core::ScopeId::new("default").unwrap());
    std::fs::create_dir_all(runs.parent().unwrap()).unwrap();
    std::fs::write(&runs, b"{}\n").unwrap();
    let view = read(&harness);
    assert_eq!(view.entries(), &expected(&[("manifest.json", b"{}")]));
}
