//! A `verifies` site the scanner recognizes but cannot read is reported with
//! its file and line instead of dropped.

use camino::Utf8PathBuf;
use provenance_macros::verifies;
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
#[verifies("rule_verification_method_words", examples)]
fn script_verifies_calls_without_a_method_warn_with_the_line() {
    let scan = scan(
        "bindings.js",
        "verifies(\"rule_pw\");\nverifies(\"rule_pw\", bogus_method);\nverifies(\"rule_pw\", examples);\nrule(\"rule_pw\", impl);\nverifies(RULE_ID, 'examples');\n",
    );

    let lines = scan
        .warnings
        .iter()
        .map(|warning| warning.line)
        .collect::<Vec<_>>();
    assert_eq!(lines, vec![1, 2, 5], "{:?}", scan.warnings);
    for warning in &scan.warnings {
        assert!(
            warning.message.contains("malformed verifies binding"),
            "{warning:?}"
        );
        assert!(warning.message.contains("examples"), "{warning:?}");
    }
    assert!(
        scan.warnings[2].message.contains("RULE_ID"),
        "{:?}",
        scan.warnings
    );
    assert_eq!(scan.bindings.len(), 2);
    assert_eq!(scan.bindings[0].line, 3);
}

#[test]
#[verifies("rule_verification_method_words", examples)]
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
#[verifies("rule_verification_method_words", examples)]
fn well_formed_sites_and_other_calls_warn_about_nothing() {
    let scan = scan(
        "clean.ts",
        "verifies('rule_pw', 'examples');\nconst verified = verifiesSomething(x);\nrule(\"rule_pw\", { statement: \"x\" });\n\
         function verifies(ruleId, method) {}\nexport function verifies(\n  _ruleId: string,\n  _method: VerificationMethod,\n): void {}\n",
    );

    assert!(scan.warnings.is_empty(), "{:?}", scan.warnings);
    assert_eq!(scan.bindings.len(), 1);
}

/// A TypeScript line that only shares the name `verifies` with the binding
/// helper gives no warning and no binding.
fn assert_not_a_binding_site(source: &str) {
    let scan = scan("other.ts", source);

    assert!(scan.warnings.is_empty(), "{source}: {:?}", scan.warnings);
    assert!(scan.bindings.is_empty(), "{source}: {:?}", scan.bindings);
}

#[test]
#[verifies("rule_verification_method_words", examples)]
fn a_class_method_named_verifies_is_not_a_binding_site() {
    assert_not_a_binding_site(
        "class Signer {\n  verifies(sig: string) {\n    return true;\n  }\n}\n",
    );
    assert_not_a_binding_site(
        "class Signer {\n  public async verifies(sig: string): Promise<boolean> {\n    return true;\n  }\n}\n",
    );
    assert_not_a_binding_site("interface Signer {\n  verifies(sig: string): boolean;\n}\n");
}

#[test]
#[verifies("rule_verification_method_words", examples)]
fn a_member_call_to_another_verifies_is_not_a_binding_site() {
    assert_not_a_binding_site("const ok = token.verifies(key);\n");
    assert_not_a_binding_site("if (signer?.verifies(\"payload\")) {}\n");
}

#[test]
#[verifies("rule_verification_method_words", examples)]
fn an_async_function_named_verifies_is_not_a_binding_site() {
    assert_not_a_binding_site("async function verifies(sig: string) {}\n");
    assert_not_a_binding_site("export async function verifies(sig: string) {}\n");
}

#[test]
#[verifies("rule_verification_method_words", examples)]
fn a_default_export_named_verifies_is_not_a_binding_site() {
    assert_not_a_binding_site("export default function verifies(sig: string) {}\n");
}

#[test]
#[verifies("rule_verification_method_words", examples)]
fn a_namespace_verifies_call_still_binds_and_a_bare_call_still_warns() {
    let scan = scan(
        "bindings.ts",
        "provenance.verifies('rule_pw', 'examples');\nverifies(sig);\n",
    );

    assert_eq!(scan.bindings.len(), 1, "{:?}", scan.bindings);
    let lines = scan
        .warnings
        .iter()
        .map(|warning| warning.line)
        .collect::<Vec<_>>();
    assert_eq!(lines, vec![2], "{:?}", scan.warnings);
}
