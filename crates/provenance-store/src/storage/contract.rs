//! The contract that every storage adapter satisfies.
//!
//! Each case takes a fresh harness. A harness seeds state by the adapter's
//! own means, not through the port, and runs each port call in the section
//! that the adapter needs.

use super::{Atomicity, PublishFailure, Revision, StateKey, StateStorage, StateView, WriteSet};
use std::collections::BTreeMap;
use std::sync::Arc;

/// One adapter under test.
pub trait Harness {
    /// Puts `entries` into the state, by the adapter's own means.
    fn seed(&mut self, entries: &[(&str, &[u8])]);
    /// Runs `call` with the adapter, in the section that the adapter needs.
    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R;
    /// Makes a later landing of `key` fail. Call it after the last seed.
    fn break_key(&mut self, key: &str);
}

fn publish<H: Harness>(harness: &H, writes: WriteSet) -> Result<(), PublishFailure> {
    harness.with_storage(|storage| storage.publish(writes))
}

fn key(key: &str) -> StateKey {
    StateKey::new(key).unwrap()
}

fn read<H: Harness>(harness: &H) -> StateView {
    harness.with_storage(|storage| storage.read_view().unwrap())
}

fn expected(entries: &[(&str, &[u8])]) -> BTreeMap<StateKey, Vec<u8>> {
    entries
        .iter()
        .map(|(key, bytes)| (StateKey::new(*key).unwrap(), bytes.to_vec()))
        .collect()
}

/// Shards of scope `a`, in key order: `A` < `B` < `C`.
const A: &str = "scopes/a/questions/question.jsonl";
const B: &str = "scopes/a/rules/rule.jsonl";
const C: &str = "scopes/a/sources/source.jsonl";
const X: &str = "scopes/a/ideation/landings.jsonl";
const Y: &str = "scopes/a/topics/topic.jsonl";

/// Bytes that a text reader would change: no final newline, CRLF, a byte
/// that is not UTF-8, and an empty file.
const SHARDS: &[(&str, &[u8])] = &[
    ("manifest.json", b"{\"scopes\":[]}"),
    (
        "scopes/default/sources/source.jsonl",
        b"{\"id\":\"a\"}\r\n{\"id\":\"b\"}",
    ),
    ("scopes/default/threads/2026-10.jsonl", b"\xff\xfe raw"),
    ("scopes/default/rules/rule.jsonl", b""),
];

/// A view holds every key with the bytes it was given, and nothing else.
pub fn view_returns_every_key_with_its_bytes<H: Harness>(mut harness: H) {
    harness.seed(SHARDS);
    let view = read(&harness);
    assert_eq!(view.entries(), &expected(SHARDS));
    assert_eq!(view.revision(), &Revision::of(&expected(SHARDS)));
}

/// An empty state reads as an empty view.
pub fn empty_state_reads_as_an_empty_view<H: Harness>(harness: &H) {
    let view = read(harness);
    assert!(view.entries().is_empty());
    assert_eq!(view.revision(), &Revision::of(&BTreeMap::new()));
}

/// Two reads of unchanged state agree; a changed byte or a moved key
/// changes the revision.
pub fn revision_follows_every_key_and_byte<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}"), (X, b"one")]);
    let first = read(&harness);
    assert_eq!(read(&harness).revision(), first.revision());

    harness.seed(&[(X, b"onf")]);
    let changed = read(&harness);
    assert_ne!(changed.revision(), first.revision());

    harness.seed(&[(Y, b"onf")]);
    assert_ne!(read(&harness).revision(), changed.revision());
}

/// A per-key set with no base lands its puts and deletes.
pub fn publish_lands_puts_and_deletes<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}"), (B, b"old")]);
    let writes = WriteSet::new(Atomicity::PerKey)
        .put(key("manifest.json"), b"{\"x\":1}".to_vec())
        .put(key(C), b"new".to_vec())
        .delete(key(B));
    publish(&harness, writes).unwrap();
    let after: &[(&str, &[u8])] = &[("manifest.json", b"{\"x\":1}"), (C, b"new")];
    assert_eq!(read(&harness).entries(), &expected(after));
}

/// A set on a view with the stored keys and bytes lands.
pub fn publish_on_the_current_base_lands<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    let writes = WriteSet::new(Atomicity::PerKey)
        .on_base(Arc::new(read(&harness)))
        .put(key(X), b"x".to_vec());
    publish(&harness, writes).unwrap();
    let after: &[(&str, &[u8])] = &[("manifest.json", b"{}"), (X, b"x")];
    assert_eq!(read(&harness).entries(), &expected(after));
}

/// A set on a view whose keys and bytes are no longer stored is refused
/// before any key lands. The failure names the revision of the base and of
/// the stored state.
pub fn publish_on_a_moved_base_is_refused<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    let base = read(&harness);
    harness.seed(&[(X, b"moved")]);
    let moved = read(&harness);
    let writes = WriteSet::new(Atomicity::PerKey)
        .on_base(Arc::new(base.clone()))
        .put(key(Y), b"y".to_vec());
    assert_base_moved(publish(&harness, writes), &base, &moved);
    assert_eq!(read(&harness), moved);
}

/// A set on a view with the stored keys, where the bytes of one key are no
/// longer stored, is refused before any key lands.
pub fn publish_on_a_base_with_changed_bytes_is_refused<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}"), (X, b"one")]);
    let base = read(&harness);
    harness.seed(&[(X, b"onf")]);
    let moved = read(&harness);
    assert!(moved.entries().keys().eq(base.entries().keys()));
    let writes = WriteSet::new(Atomicity::PerKey)
        .on_base(Arc::new(base.clone()))
        .put(key(Y), b"y".to_vec());
    assert_base_moved(publish(&harness, writes), &base, &moved);
    assert_eq!(read(&harness), moved);
}

/// A whole set on a view whose keys and bytes are no longer stored is
/// refused before any key lands.
pub fn a_whole_set_on_a_moved_base_is_refused<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}"), (A, b"a")]);
    let base = read(&harness);
    harness.seed(&[(A, b"moved")]);
    let moved = read(&harness);
    let writes = WriteSet::new(Atomicity::WholeSet)
        .on_base(Arc::new(base.clone()))
        .put(key(A), b"new".to_vec())
        .put(key(B), b"b".to_vec())
        .delete(key("manifest.json"));
    assert_base_moved(publish(&harness, writes), &base, &moved);
    assert_eq!(read(&harness), moved);
}

/// The failure names the revision of the base and of the stored state.
fn assert_base_moved(result: Result<(), PublishFailure>, base: &StateView, moved: &StateView) {
    match result {
        Err(PublishFailure::BaseMoved {
            base: from,
            current,
        }) => {
            assert_eq!(&from, base.revision());
            assert_eq!(&current, moved.revision());
        }
        other => panic!("expected BaseMoved, got {other:?}"),
    }
}

/// A set with no key on the current base changes nothing.
pub fn an_empty_set_changes_nothing<H: Harness>(mut harness: H) -> H {
    harness.seed(&[("manifest.json", b"{}"), (X, b"x")]);
    let before = read(&harness);
    for atomicity in [Atomicity::PerKey, Atomicity::WholeSet] {
        let writes = WriteSet::new(atomicity).on_base(Arc::new(before.clone()));
        publish(&harness, writes).unwrap();
        assert_eq!(read(&harness), before);
    }
    harness
}

/// A set with no key on a view whose keys and bytes are no longer stored
/// is refused.
pub fn an_empty_set_on_a_moved_base_is_refused<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    let base = read(&harness);
    harness.seed(&[(X, b"moved")]);
    let moved = read(&harness);
    for atomicity in [Atomicity::PerKey, Atomicity::WholeSet] {
        let writes = WriteSet::new(atomicity).on_base(Arc::new(base.clone()));
        assert_base_moved(publish(&harness, writes), &base, &moved);
        assert_eq!(read(&harness), moved);
    }
}

/// A set that puts and deletes one key is refused before any key lands.
pub fn publish_refuses_a_put_and_delete_of_one_key<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    let before = read(&harness);
    let writes = WriteSet::new(Atomicity::PerKey)
        .put(key(X), b"x".to_vec())
        .delete(key(X));
    assert!(matches!(
        publish(&harness, writes),
        Err(PublishFailure::Refused(_))
    ));
    assert_eq!(read(&harness), before);
}

/// A one-key set lands all of the new bytes of its key.
pub fn a_one_key_set_lands_all_of_its_bytes<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}"), (X, b"short")]);
    let bytes: &[u8] = b"{\"id\":\"a longer line\"}\r\n\xff\xfe\n";
    let writes = WriteSet::new(Atomicity::PerKey).put(key(X), bytes.to_vec());
    publish(&harness, writes).unwrap();
    let after: &[(&str, &[u8])] = &[("manifest.json", b"{}"), (X, bytes)];
    assert_eq!(read(&harness).entries(), &expected(after));
}

/// A one-key set that fails while it lands leaves the state as it was and
/// is refused.
pub fn a_failed_one_key_set_lands_nothing<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    harness.break_key(X);
    let before = read(&harness);
    let writes = WriteSet::new(Atomicity::PerKey).put(key(X), b"x".to_vec());
    assert!(matches!(
        publish(&harness, writes),
        Err(PublishFailure::Refused(_))
    ));
    assert_eq!(read(&harness), before);
}

/// A per-key set that fails at its first key changes no key and is refused.
pub fn a_per_key_set_that_fails_at_its_first_key_is_refused<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    harness.break_key(A);
    let before = read(&harness);
    let writes = WriteSet::new(Atomicity::PerKey)
        .put(key(A), b"a".to_vec())
        .put(key(B), b"b".to_vec());
    assert!(matches!(
        publish(&harness, writes),
        Err(PublishFailure::Refused(_))
    ));
    assert_eq!(read(&harness), before);
}

/// A per-key set that fails partway leaves the keys before the failure
/// landed and reports that landing started.
pub fn a_failed_per_key_set_leaves_a_landed_prefix<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    harness.break_key(B);
    let mut prefix = read(&harness).into_entries();
    let writes = WriteSet::new(Atomicity::PerKey)
        .put(key(A), b"a".to_vec())
        .put(key(B), b"b".to_vec())
        .put(key(C), b"c".to_vec());
    assert!(matches!(
        publish(&harness, writes),
        Err(PublishFailure::Started(_))
    ));
    prefix.insert(key(A), b"a".to_vec());
    assert_eq!(read(&harness).entries(), &prefix);
}

/// A whole set of two keys lands both keys.
pub fn a_two_key_whole_set_lands_both_keys<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    let writes = WriteSet::new(Atomicity::WholeSet)
        .put(key(A), b"a".to_vec())
        .put(key(B), b"b".to_vec());
    publish(&harness, writes).unwrap();
    let after: &[(&str, &[u8])] = &[("manifest.json", b"{}"), (A, b"a"), (B, b"b")];
    assert_eq!(read(&harness).entries(), &expected(after));
}

/// A whole set of several keys that fails partway rolls back: no key lands.
///
/// The local adapter lands the set on a copy of the state directory, so a
/// key that fails leaves the live directory as it was. Another adapter
/// must give the same result: when one key of the set fails, no key of
/// the set lands.
pub fn a_failed_whole_set_lands_no_key<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    harness.break_key(B);
    let before = read(&harness);
    let writes = WriteSet::new(Atomicity::WholeSet)
        .put(key(A), b"a".to_vec())
        .put(key(B), b"b".to_vec())
        .put(key(C), b"c".to_vec());
    assert!(publish(&harness, writes).is_err());
    assert_eq!(read(&harness), before);
}

#[cfg(all(test, feature = "host"))]
mod local_harness;

#[cfg(all(test, feature = "host"))]
mod tests {
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
}

#[cfg(test)]
mod persisting;
