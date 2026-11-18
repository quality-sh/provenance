//! The atomicity of a set: a per-key set lands its keys in key order up to
//! a failure or lands no key, a whole set lands every key or none, and a set
//! that puts and deletes one key is refused.

use super::{
    expected, key, publish, read, Atomicity, Harness, PublishFailure, StateView, WriteSet, A, B, C,
    X,
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

/// A whole set of several keys that fails at one key is never seen in part.
///
/// The adapter refuses the set with no key changed, lands it, or reports
/// that it started with an unknown outcome. Each read in the section of the
/// write gives the state before the set or the whole set, or fails. When
/// the cause of the failure is gone, the next section gives the state
/// before a refused set, the whole state of a set that landed, and one of
/// the two for a started set, the same that a read in the section gave.
/// Thus no read sees the set in part, and the set is never left in part.
///
/// The local adapter refuses the set before its marker when the live entry
/// of a key is not a file. On Windows, a file that a different process holds
/// makes the install fail after the marker: the write succeeds, a read in
/// its section completes the install first or fails, and the next section
/// entry completes it.
pub fn a_failed_whole_set_is_never_seen_in_part<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}"), (B, b"old")]);
    harness.break_key(B);
    let before = read(&harness).into_entries();
    let writes = WriteSet::new(Atomicity::WholeSet)
        .put(key(A), b"a".to_vec())
        .put(key(B), b"b".to_vec())
        .put(key(C), b"c".to_vec());
    let (landed, seen) =
        harness.with_storage(|storage| (storage.publish(writes), storage.read_view()));
    let seen = seen.ok().map(StateView::into_entries);
    let mut whole = before.clone();
    whole.extend(expected(&[(A, b"a"), (B, b"b"), (C, b"c")]));
    let allowed = match landed {
        Err(PublishFailure::Refused(_)) => vec![&before],
        Ok(()) => vec![&whole],
        Err(PublishFailure::Started(_)) => vec![&before, &whole],
        Err(PublishFailure::BaseMoved { .. }) => {
            panic!("a set with no base is not refused for its base")
        }
    };
    if let Some(seen) = &seen {
        assert!(
            allowed.contains(&seen),
            "a read in the section sees the set in part"
        );
    }
    harness.repair_key(B);
    let after = read(&harness).into_entries();
    assert!(
        allowed.contains(&&after),
        "the next section leaves the set in part"
    );
    assert!(
        seen.is_none_or(|seen| seen == after),
        "the next section changes what a read in the section saw"
    );
}
