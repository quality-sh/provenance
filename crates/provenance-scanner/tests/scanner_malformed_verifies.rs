//! A `verifies` site the scanner recognizes but cannot read is reported with
//! its file and line instead of dropped.

use camino::Utf8PathBuf;
use provenance_scanner::scan_path_with_content;

fn scan(name: &str, source: &str) -> provenance_scanner::FileScan {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join(name), source).unwrap();
    let path = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let mut scanned = scan_path_with_content(&path).unwrap();
    assert_eq!(scanned.len(), 1);
    scanned.remove(0).scan
}

#[test]
fn script_verifies_calls_without_a_method_warn_with_the_line() {
    let scan = scan(
        "bindings.js",
        "verifies(\"rule_pw\");\nverifies(\"rule_pw\", bogus_method);\nverifies(\"rule_pw\", examples);\nrule(\"rule_pw\", impl);\n",
    );

    let lines = scan
        .warnings
        .iter()
        .map(|warning| warning.line)
        .collect::<Vec<_>>();
    assert_eq!(lines, vec![1, 2], "{:?}", scan.warnings);
    for warning in &scan.warnings {
        assert!(
            warning.message.contains("malformed verifies binding"),
            "{warning:?}"
        );
        assert!(warning.message.contains("rule_pw"), "{warning:?}");
        assert!(warning.message.contains("examples"), "{warning:?}");
    }
    assert_eq!(scan.bindings.len(), 2);
    assert_eq!(scan.bindings[0].line, 3);
}

#[test]
fn rust_verifies_attributes_without_a_method_warn_with_the_line() {
    let scan = scan(
        "lib.rs",
        "#[verifies(\"rule_pw\")]\nfn one() {}\n#[verifies(\"rule_pw\", bogus)]\nfn two() {}\n#[verifies(\"rule_pw\", property)]\nfn three() {}\n#[rule(\"rule_pw\")]\nfn four() {}\n",
    );

    let lines = scan
        .warnings
        .iter()
        .map(|warning| warning.line)
        .collect::<Vec<_>>();
    assert_eq!(lines, vec![1, 3], "{:?}", scan.warnings);
    assert_eq!(scan.bindings.len(), 2);
}

#[test]
fn well_formed_sites_and_other_calls_warn_about_nothing() {
    let scan = scan(
        "clean.ts",
        "verifies('rule_pw', 'examples');\nconst verified = verifiesSomething(x);\nrule(\"rule_pw\", { statement: \"x\" });\n",
    );

    assert!(scan.warnings.is_empty(), "{:?}", scan.warnings);
    assert_eq!(scan.bindings.len(), 1);
}
