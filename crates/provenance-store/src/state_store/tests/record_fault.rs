//! A fault over stored records reads the shard again only to find the line
//! of the record. When that second read fails or does not find the id, the
//! refusal is the first fault with no line, and the second error is not
//! given.
//!
//! The first read of a shard refuses a bad line first, so the tests make
//! these two shard states with a direct call. No test shows that a public
//! surface reaches the fallback, and no test shows that none can. No Rule
//! states this fallback.

use crate::state_store::readers::{record_fault, StateLineInvalid};

const MESSAGE: &str = "requirement req_a does not agree with its Rule";

fn fault_over(shard: &str) -> anyhow::Error {
    let (_dir, store, scope) = super::initialized_store();
    let path = crate::shards::requirements_path(&store.layout, &scope);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, shard).unwrap();
    record_fault(&store, &path, "req_a", MESSAGE.into())
}

#[test]
fn a_second_read_that_fails_gives_the_first_fault_with_no_line() {
    let error = fault_over("not a record\n");
    assert!(StateLineInvalid::find(&error).is_none(), "{error:#}");
    assert_eq!(format!("{error:#}"), MESSAGE);
}

#[test]
fn a_second_read_that_does_not_find_the_id_gives_the_first_fault_with_no_line() {
    let error = fault_over("{\"id\":\"req_b\"}\n");
    assert!(StateLineInvalid::find(&error).is_none(), "{error:#}");
    assert_eq!(format!("{error:#}"), MESSAGE);
}
