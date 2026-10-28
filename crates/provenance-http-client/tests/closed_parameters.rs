//! The generated parameter enums are the whole selection: an invented value
//! has no variant and no string conversion, so a bad call cannot compile.

#[test]
fn invented_parameter_values_cannot_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/closed_parameters/invented_direction.rs");
    cases.compile_fail("tests/closed_parameters/invented_side.rs");
}
