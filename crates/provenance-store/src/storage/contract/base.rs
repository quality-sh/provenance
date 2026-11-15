//! The base of a set: a set on the stored keys and bytes lands, and a set
//! on a base that moved is refused before any key lands, whatever its keys.

use super::{
    expected, key, publish, read, Arc, Atomicity, Harness, PublishFailure, StateView, WriteSet, A,
    B, X, Y,
};

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

/// A set on a view whose keys and bytes are not the stored ones is refused
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

/// A set on a view with the stored keys, where the bytes of one key are not
/// the stored bytes, is refused before any key lands.
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

/// A whole set on a view whose keys and bytes are not the stored ones is
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

/// A set with no key on a view whose keys and bytes are not the stored ones
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
