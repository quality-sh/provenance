//! The `verifies` item checks hold through the real compiler: a
//! `construction` marker on a function must fail to compile, and one on a
//! type must compile.

#[test]
fn construction_refuses_functions_and_accepts_type_forms() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/construction_on_function.rs");
    cases.compile_fail("tests/compile_fail/construction_on_test_function.rs");
    cases.pass("tests/compile_pass/construction_on_struct.rs");
    cases.pass("tests/compile_pass/construction_on_enum.rs");
}
