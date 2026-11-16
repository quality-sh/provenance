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
    /// Makes each later write of `key` fail in the storage that the adapter
    /// writes to, whichever set makes the write. A check that the adapter
    /// makes only for the keys of a set is not enough: the cases that break
    /// each stored key catch an adapter that writes a key outside its set
    /// only when that write fails. A read of the state still succeeds after
    /// it. A case calls it for more than one key in turn, and each key stays
    /// broken. Call it after the last seed.
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
const X: &str = "scopes/a/ideation/contributions.jsonl";
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

mod atomicity;
mod base;
mod empty_set;
mod one_key;
mod view;

pub use atomicity::{
    a_failed_per_key_set_leaves_a_landed_prefix, a_failed_whole_set_lands_no_key,
    a_per_key_set_that_fails_at_its_first_key_is_refused, a_two_key_whole_set_lands_both_keys,
    publish_lands_puts_and_deletes, publish_refuses_a_put_and_delete_of_one_key,
};
pub use base::{
    a_whole_set_on_a_moved_base_is_refused, an_empty_set_on_a_moved_base_is_refused,
    publish_on_a_base_with_changed_bytes_is_refused, publish_on_a_moved_base_is_refused,
    publish_on_the_current_base_lands,
};
pub use empty_set::{
    an_empty_set_changes_nothing, an_empty_set_lands_while_each_stored_key_is_broken,
};
pub use one_key::{
    a_failed_one_key_set_lands_nothing, a_one_key_set_lands_all_of_its_bytes,
    a_one_key_set_lands_while_each_other_key_is_broken,
};
pub use view::{
    empty_state_reads_as_an_empty_view, revision_follows_every_key_and_byte,
    view_returns_every_key_with_its_bytes,
};

#[cfg(all(test, feature = "host"))]
mod local_harness;

#[cfg(all(test, feature = "host"))]
mod tests;

#[cfg(test)]
mod persisting;

#[cfg(test)]
mod whole_landing;
