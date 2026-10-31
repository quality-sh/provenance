//! The contract that every storage adapter satisfies.
//!
//! Each case takes a fresh harness. A harness seeds state by the adapter's
//! own means, not through the port, and runs each port call in the section
//! that the adapter needs.

use super::{Revision, StateKey, StateStorage, StateView};
use std::collections::BTreeMap;

/// One adapter under test.
pub trait Harness {
    /// Puts `entries` into the state, by the adapter's own means.
    fn seed(&mut self, entries: &[(&str, &[u8])]);
    /// Runs `call` with the adapter, in the section that the adapter needs.
    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R;
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
pub fn empty_state_reads_as_an_empty_view<H: Harness>(harness: H) {
    let view = read(&harness);
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
        empty_state_reads_as_an_empty_view(local());
    }

    #[test]
    fn memory_empty_state_reads_as_an_empty_view() {
        empty_state_reads_as_an_empty_view(Memory::default());
    }

    #[test]
    fn local_revision_follows_every_key_and_byte() {
        revision_follows_every_key_and_byte(local());
    }

    #[test]
    fn memory_revision_follows_every_key_and_byte() {
        revision_follows_every_key_and_byte(Memory::default());
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
