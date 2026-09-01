//! Compile-fail fixtures for capability types. Each case pins its exact
//! diagnostic, so an unrelated compile error cannot keep a fixture green.

#[test]
fn capability_refusals_hold_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/forged_guard.rs");
}
