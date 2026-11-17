//! The atomicity of a set: a per-key set lands its keys in key order up to
//! a failure or lands no key, a whole set lands every key or none, and a set
//! that puts and deletes one key is refused.

use super::{
    expected, key, publish, read, Atomicity, Harness, PublishFailure, WriteSet, A, B, C, X,
};

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

/// A per-key set that fails at a later key leaves landed the keys before
/// the failure, or no key. It is refused when no key landed and reports
/// that landing started otherwise.
pub fn a_failed_per_key_set_leaves_a_landed_prefix<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}")]);
    harness.break_key(B);
    let before = read(&harness).into_entries();
    let writes = WriteSet::new(Atomicity::PerKey)
        .put(key(A), b"a".to_vec())
        .put(key(B), b"b".to_vec())
        .put(key(C), b"c".to_vec());
    let failure = publish(&harness, writes).unwrap_err();
    let after = read(&harness).into_entries();
    match failure {
        PublishFailure::Refused(_) => assert_eq!(after, before, "a refused set lands no key"),
        PublishFailure::Started(_) => {
            let mut prefix = before;
            prefix.insert(key(A), b"a".to_vec());
            assert_eq!(
                after, prefix,
                "a started set lands the keys before the failure"
            );
        }
        PublishFailure::BaseMoved { .. } => {
            panic!("a set with no base is not refused for its base")
        }
    }
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
