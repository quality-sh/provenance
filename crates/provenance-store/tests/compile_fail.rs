//! Compile-fail fixtures. Each case pins its exact diagnostic.

#[test]
fn capability_refusals_hold_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/forged_guard.rs");
    cases.compile_fail("tests/compile_fail/read_after_stamp.rs");
}
