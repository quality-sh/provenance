//! The `verifies` item checks hold through the real compiler: a
//! `construction` marker on a function must fail to compile, and one on a
//! type must compile.
//!
//! The fixture sources sit in `fixtures/provenance-macros/`, outside the
//! `crates/` tree the strict Rule Coverage command scans. They cite a
//! demonstration rule id, and the packages that exercise marker machinery
//! keep such fixtures outside `crates/` for the same reason.

#[test]
fn construction_refuses_functions_and_accepts_type_forms() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("../../fixtures/provenance-macros/construction_on_function.rs");
    cases.compile_fail("../../fixtures/provenance-macros/construction_on_test_function.rs");
    cases.pass("../../fixtures/provenance-macros/construction_on_struct.rs");
    cases.pass("../../fixtures/provenance-macros/construction_on_enum.rs");
}
