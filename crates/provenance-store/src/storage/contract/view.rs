//! The read view: each key with its bytes, and a revision that follows
//! each key and byte.

use super::{expected, read, BTreeMap, Harness, Revision, SHARDS, X, Y};

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
