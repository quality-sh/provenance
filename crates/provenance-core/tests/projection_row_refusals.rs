//! Compile-fail fixtures for `#[derive(ProjectionRow)]`. Each case pins its
//! exact diagnostic.

#[test]
fn a_tuple_struct_is_refused() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/projection_row_tuple_struct.rs");
}

#[test]
fn a_field_named_search_text_is_refused() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/compile_fail/projection_row_search_text.rs");
}
