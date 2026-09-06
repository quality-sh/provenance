//! Compile-fail fixtures. Each case pins its exact diagnostic.

use provenance_macros::verifies;

#[test]
#[verifies("rule_store_under_guard_takes_no_second_lock", examples)]
fn capability_refusals_hold_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/forged_guard.rs");
    cases.compile_fail("tests/compile_fail/guarded_store_outlives_guard.rs");
    cases.compile_fail("tests/compile_fail/read_after_stamp.rs");
}
