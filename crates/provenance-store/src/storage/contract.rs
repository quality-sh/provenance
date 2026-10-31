//! The contract that every storage adapter satisfies.
//!
//! Each case takes a fresh harness. A harness seeds state by the adapter's
//! own means, not through the port, and runs each port call in the section
//! that the adapter needs.

use super::{Atomicity, PublishFailure, Revision, StateKey, StateStorage, StateView, WriteSet};
use std::collections::BTreeMap;

/// One adapter under test.
pub trait Harness {
    /// Puts `entries` into the state, by the adapter's own means.
    fn seed(&mut self, entries: &[(&str, &[u8])]);
    /// Runs `call` with the adapter, in the section that the adapter needs.
    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R;
    /// Makes a later landing of `key` fail. Call it after the last seed.
    fn break_key(&mut self, key: &str);
}

fn publish<H: Harness>(harness: &H, writes: WriteSet) -> Result<Revision, PublishFailure> {
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
    harness.seed(&[("manifest.json", b"{}"), ("scopes/a/x.jsonl", b"one")]);
    let first = read(&harness);
    assert_eq!(read(&harness).revision(), first.revision());

    harness.seed(&[("scopes/a/x.jsonl", b"onf")]);
    let changed = read(&harness);
    assert_ne!(changed.revision(), first.revision());

    harness.seed(&[("scopes/a/y.jsonl", b"onf")]);
    assert_ne!(read(&harness).revision(), changed.revision());
}

/// A per-key set lands its puts and deletes, and returns the revision of the
/// whole tree after the write.
///
/// The set has no base, so an adapter reads and
/// hashes the whole tree after the write lands; the local adapter does this
/// under the publication lock.
pub fn publish_lands_puts_and_deletes<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}"), ("scopes/a/b.jsonl", b"old")]);
    let writes = WriteSet::new(Atomicity::PerKey)
        .put(key("manifest.json"), b"{\"x\":1}".to_vec())
        .put(key("scopes/a/c.jsonl"), b"new".to_vec())
        .delete(key("scopes/a/b.jsonl"));
    let revision = publish(&harness, writes).unwrap();
    let view = read(&harness);
    let after: &[(&str, &[u8])] = &[
        ("manifest.json", b"{\"x\":1}"),
        ("scopes/a/c.jsonl", b"new"),
    ];
    assert_eq!(view.entries(), &expected(after));
    assert_eq!(&revision, view.revision());
}

/// A set on the current revision lands and returns the revision of the whole
/// tree after it. The adapter has the tree from its read of the base.
pub fn publish_on_the_current_base_lands<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    let base = read(&harness).revision().clone();
    let writes = WriteSet::new(Atomicity::PerKey)
        .on_base(base)
        .put(key("scopes/a/x.jsonl"), b"x".to_vec());
    let revision = publish(&harness, writes).unwrap();
    assert_eq!(&revision, read(&harness).revision());
}

/// A set on a revision that moved is refused before any key lands.
pub fn publish_on_a_moved_base_is_refused<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    let base = read(&harness).revision().clone();
    harness.seed(&[("scopes/a/x.jsonl", b"moved")]);
    let moved = read(&harness);
    let writes = WriteSet::new(Atomicity::PerKey)
        .on_base(base.clone())
        .put(key("scopes/a/y.jsonl"), b"y".to_vec());
    match publish(&harness, writes) {
        Err(PublishFailure::BaseMoved {
            base: from,
            current,
        }) => {
            assert_eq!(from, base);
            assert_eq!(&current, moved.revision());
        }
        other => panic!("expected BaseMoved, got {other:?}"),
    }
    assert_eq!(read(&harness), moved);
}

/// A set that puts and deletes one key is refused before any key lands.
pub fn publish_refuses_a_put_and_delete_of_one_key<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    let before = read(&harness);
    let writes = WriteSet::new(Atomicity::PerKey)
        .put(key("scopes/a/x.jsonl"), b"x".to_vec())
        .delete(key("scopes/a/x.jsonl"));
    assert!(matches!(
        publish(&harness, writes),
        Err(PublishFailure::Refused(_))
    ));
    assert_eq!(read(&harness), before);
}

/// A one-key set that fails while it lands leaves the state as it was and
/// reports that landing started.
pub fn a_failed_one_key_set_lands_nothing<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    harness.break_key("scopes/a/x.jsonl");
    let before = read(&harness);
    let writes = WriteSet::new(Atomicity::PerKey).put(key("scopes/a/x.jsonl"), b"x".to_vec());
    assert!(matches!(
        publish(&harness, writes),
        Err(PublishFailure::Started(_))
    ));
    assert_eq!(read(&harness), before);
}

/// A per-key set that fails partway leaves the keys before the failure
/// landed and reports that landing started.
pub fn a_failed_per_key_set_leaves_a_landed_prefix<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    harness.break_key("scopes/a/b.jsonl");
    let mut prefix = read(&harness).into_entries();
    let writes = WriteSet::new(Atomicity::PerKey)
        .put(key("scopes/a/a.jsonl"), b"a".to_vec())
        .put(key("scopes/a/b.jsonl"), b"b".to_vec())
        .put(key("scopes/a/c.jsonl"), b"c".to_vec());
    assert!(matches!(
        publish(&harness, writes),
        Err(PublishFailure::Started(_))
    ));
    prefix.insert(key("scopes/a/a.jsonl"), b"a".to_vec());
    assert_eq!(read(&harness).entries(), &prefix);
}

/// A whole set of two keys lands both keys. Only an adapter that lands
/// whole sets runs this case; the local adapter refuses the set.
pub fn a_two_key_whole_set_lands_both_keys<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    let writes = WriteSet::new(Atomicity::WholeSet)
        .put(key("scopes/a/a.jsonl"), b"a".to_vec())
        .put(key("scopes/a/b.jsonl"), b"b".to_vec());
    let revision = publish(&harness, writes).unwrap();
    let view = read(&harness);
    let after: &[(&str, &[u8])] = &[
        ("manifest.json", b"{}"),
        ("scopes/a/a.jsonl", b"a"),
        ("scopes/a/b.jsonl", b"b"),
    ];
    assert_eq!(view.entries(), &expected(after));
    assert_eq!(&revision, view.revision());
}

/// A whole set of several keys that fails partway rolls back: no key lands.
///
/// The local adapter cannot run this case. It refuses a whole set of more
/// than one key before any key lands, until the staged publication path
/// moves behind the port. The Durable Object adapter must run it: a
/// statement that fails inside its transaction rolls back every earlier
/// statement of the set.
pub fn a_failed_whole_set_lands_no_key<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    harness.break_key("scopes/a/b.jsonl");
    let before = read(&harness);
    let writes = WriteSet::new(Atomicity::WholeSet)
        .put(key("scopes/a/a.jsonl"), b"a".to_vec())
        .put(key("scopes/a/b.jsonl"), b"b".to_vec())
        .put(key("scopes/a/c.jsonl"), b"c".to_vec());
    assert!(publish(&harness, writes).is_err());
    assert_eq!(read(&harness), before);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::ProvenanceLayout;
    use crate::storage::local::LocalStateStorage;
    use crate::storage::memory::MemoryStateStorage;
    use camino::Utf8Path;
    use provenance_macros::verifies;

    struct Local {
        _temp: tempfile::TempDir,
        layout: ProvenanceLayout,
        storage: LocalStateStorage,
    }

    fn local() -> Local {
        let temp = tempfile::tempdir().unwrap();
        let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
        Local {
            storage: LocalStateStorage::new(layout.clone()),
            layout,
            _temp: temp,
        }
    }

    impl Harness for Local {
        fn seed(&mut self, entries: &[(&str, &[u8])]) {
            for (key, bytes) in entries {
                let path = self.layout.state_dir().join(key);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, bytes).unwrap();
            }
        }

        fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R {
            crate::publication::with_repository_publication(&self.layout, |_| {
                Ok(call(&self.storage))
            })
            .unwrap()
        }

        /// A directory in the place of the file makes its rename fail.
        fn break_key(&mut self, key: &str) {
            let path = self.layout.state_dir().join(key);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("keep"), b"").unwrap();
        }
    }

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
    fn both_adapters_publish_lands_puts_and_deletes() {
        publish_lands_puts_and_deletes(local());
        publish_lands_puts_and_deletes(Memory::default());
    }

    #[test]
    #[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
    fn both_adapters_publish_on_the_current_base_lands() {
        publish_on_the_current_base_lands(local());
        publish_on_the_current_base_lands(Memory::default());
    }

    #[test]
    #[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
    fn both_adapters_publish_on_a_moved_base_is_refused() {
        publish_on_a_moved_base_is_refused(local());
        publish_on_a_moved_base_is_refused(Memory::default());
    }

    #[test]
    #[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
    fn both_adapters_publish_refuses_a_put_and_delete_of_one_key() {
        publish_refuses_a_put_and_delete_of_one_key(local());
        publish_refuses_a_put_and_delete_of_one_key(Memory::default());
    }

    #[test]
    #[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
    fn both_adapters_a_failed_one_key_set_lands_nothing() {
        a_failed_one_key_set_lands_nothing(local());
        a_failed_one_key_set_lands_nothing(Memory::default());
    }

    #[test]
    #[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
    fn both_adapters_a_failed_per_key_set_leaves_a_landed_prefix() {
        a_failed_per_key_set_leaves_a_landed_prefix(local());
        a_failed_per_key_set_leaves_a_landed_prefix(Memory::default());
    }

    #[test]
    #[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
    fn memory_lands_a_two_key_whole_set_and_rolls_back_a_failed_one() {
        a_two_key_whole_set_lands_both_keys(Memory::default());
        a_failed_whole_set_lands_no_key(Memory::default());
    }

    /// The local adapter cannot run the whole-set cases; see
    /// [`a_failed_whole_set_lands_no_key`]. It refuses the set instead, and
    /// no key lands.
    #[test]
    #[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
    fn local_refuses_a_multi_key_whole_set() {
        let mut harness = local();
        harness.seed(&[("manifest.json", b"{}")]);
        let before = read(&harness);
        let writes = WriteSet::new(Atomicity::WholeSet)
            .put(key("scopes/a/a.jsonl"), b"a".to_vec())
            .put(key("scopes/a/b.jsonl"), b"b".to_vec());
        assert!(matches!(
            publish(&harness, writes),
            Err(PublishFailure::Refused(_))
        ));
        assert_eq!(read(&harness), before);
    }

    #[test]
    #[verifies("rule_canonical_state_is_read_as_one_view", examples)]
    fn local_view_returns_every_key_with_its_bytes() {
        view_returns_every_key_with_its_bytes(local());
    }

    #[test]
    #[verifies("rule_canonical_state_is_read_as_one_view", examples)]
    fn memory_view_returns_every_key_with_its_bytes() {
        view_returns_every_key_with_its_bytes(Memory::default());
    }

    #[test]
    fn local_empty_state_reads_as_an_empty_view() {
        empty_state_reads_as_an_empty_view(&local());
    }

    #[test]
    fn memory_empty_state_reads_as_an_empty_view() {
        empty_state_reads_as_an_empty_view(&Memory::default());
    }

    #[test]
    fn local_revision_follows_every_key_and_byte() {
        revision_follows_every_key_and_byte(local());
    }

    #[test]
    fn memory_revision_follows_every_key_and_byte() {
        revision_follows_every_key_and_byte(Memory::default());
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
