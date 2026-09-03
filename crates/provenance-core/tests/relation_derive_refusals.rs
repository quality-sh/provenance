//! Compile-fail fixtures for `#[derive(Relations)]`. Each case pins its
//! exact diagnostic.

#[test]
fn relation_declaration_refusals_hold_at_compile_time() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/bad_field_type.rs");
    cases.compile_fail("tests/compile_fail/unknown_key.rs");
    cases.compile_fail("tests/compile_fail/missing_target.rs");
    cases.compile_fail("tests/compile_fail/undeclared_stable_id.rs");
}
