//! A set of one key: it lands all of its bytes or none, and it writes no
//! other key.

use super::{
    expected, key, publish, read, Arc, Atomicity, Harness, PublishFailure, WriteSet, A, B, C, X,
};

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

/// A per-key set of one key lands, with no base and on the current base,
/// while each other stored key in turn is broken. So the adapter writes no
/// other key for a set of one key.
pub fn a_one_key_set_lands_while_each_other_key_is_broken<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}"), (A, b"a"), (B, b"b"), (C, b"c")]);
    for (turn, broken) in ["manifest.json", A, B, C].into_iter().enumerate() {
        harness.break_key(broken);
        for on_base in [false, true] {
            let before = read(&harness);
            let bytes = format!("{turn} {on_base}").into_bytes();
            let mut writes = WriteSet::new(Atomicity::PerKey).put(key(X), bytes.clone());
            if on_base {
                writes = writes.on_base(Arc::new(before.clone()));
            }
            let landed = publish(&harness, writes);
            assert!(
                landed.is_ok(),
                "a set of one key wrote the broken key {broken}: {landed:?}"
            );
            let mut after = before.entries().clone();
            after.insert(key(X), bytes);
            assert_eq!(read(&harness).entries(), &after);
        }
    }
}
