//! Compile-fail fixtures for the type projection (V4). PARITY.md maps
//! these to the TypeScript type-fixture suite.

#[test]
fn type_projection_refusals_hold_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/*.rs");
}
