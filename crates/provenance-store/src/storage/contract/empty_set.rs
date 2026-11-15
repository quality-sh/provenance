//! A set with no key: it lands and changes nothing, and it writes no
//! stored key.

use super::{publish, read, Arc, Atomicity, Harness, WriteSet, A, B, X};

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

/// A set with no key lands, with no base and on the current base, while each
/// stored key in turn is broken. So the adapter writes no stored key again
/// for a set with no key.
pub fn an_empty_set_lands_while_each_stored_key_is_broken<H: Harness>(mut harness: H) {
    harness.seed(&[("manifest.json", b"{}"), (A, b"a"), (B, b"b"), (X, b"x")]);
    for broken in ["manifest.json", A, B, X] {
        harness.break_key(broken);
        let before = read(&harness);
        for atomicity in [Atomicity::PerKey, Atomicity::WholeSet] {
            for on_base in [false, true] {
                let mut writes = WriteSet::new(atomicity);
                if on_base {
                    writes = writes.on_base(Arc::new(before.clone()));
                }
                let landed = publish(&harness, writes);
                assert!(
                    landed.is_ok(),
                    "a {atomicity:?} set with no key wrote the broken key {broken}: {landed:?}"
                );
                assert_eq!(read(&harness), before);
            }
        }
    }
}
