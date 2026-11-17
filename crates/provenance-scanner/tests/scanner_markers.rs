//! Public scanner examples for the comment marker that starts a directive.
use camino::Utf8Path;
use provenance_macros::verifies;
use provenance_scanner::{scan_file, Language};

/// Only `@provenance` starts a directive. A source file with a `@statesman`
/// line gives no annotation, no binding and no warning.
#[verifies("rule_prov_annot_010", examples)]
#[test]
fn statesman_marker_in_a_source_file_gives_no_binding() {
    let scan = scan_file(
        Utf8Path::new("payroll.rs"),
        Language::Rust,
        "// @statesman rule: SCHADS-PAY-001\nfn pays_overtime() {}\n",
    );

    assert!(scan.annotations.is_empty());
    assert!(scan.bindings.is_empty());
    assert!(scan.warnings.is_empty());
}

/// The same line with the `@provenance` marker binds the function.
#[verifies("rule_prov_annot_010", examples)]
#[test]
fn provenance_marker_in_a_source_file_gives_an_annotation() {
    let scan = scan_file(
        Utf8Path::new("payroll.rs"),
        Language::Rust,
        "// @provenance rule: SCHADS-PAY-001\nfn pays_overtime() {}\n",
    );

    assert_eq!(scan.annotations.len(), 1);
    assert_eq!(scan.annotations[0].annotation.rule, "SCHADS-PAY-001");
    assert_eq!(
        scan.annotations[0].function_name.as_deref(),
        Some("pays_overtime")
    );
    assert!(scan.warnings.is_empty());
}
