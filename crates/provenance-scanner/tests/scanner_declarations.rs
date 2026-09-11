//! Typed Rule declaration calls never become implementation bindings; the
//! free identity helpers from the `rules` subpath still bind, and unknown
//! ids on real bindings still warn.

use camino::Utf8Path;
use provenance_scanner::{scan_file, validate_bindings, Language, Verification};

#[test]
fn typed_rule_declaration_call_is_not_an_implementation_binding() {
    let scan = scan_file(
        Utf8Path::new("index.test.ts"),
        Language::TypeScript,
        r#"const fixture = requirement.rule("expiry", {statement: "Expires after 30 days"});"#,
    );

    assert!(scan.bindings.is_empty());
}

#[test]
fn tracked_sdk_declaration_shapes_never_bind() {
    for source in [
        r#"const expiry = sharing.rule("expiry", {
  statement: "Share links expire within 30 days",
});"#,
        r#"expiry: sharing.rule("expiry", { statement: "Share links expire" });"#,
        r#"const shareExpiry = sharing.rule("expiry").statement("Share links expire");"#,
    ] {
        let scan = scan_file(Utf8Path::new("spec.ts"), Language::TypeScript, source);

        assert!(scan.bindings.is_empty(), "{source}");
    }
}

#[test]
fn free_rule_helper_call_remains_a_binding() {
    let scan = scan_file(
        Utf8Path::new("rules.ts"),
        Language::TypeScript,
        r#"import { rule } from "@quality-sh/provenance/rules";

export const paysOvertime = rule("rule_overtime", (hours: number) => hours > 38);"#,
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "rule_overtime");
    assert_eq!(scan.bindings[0].verification, None);
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("paysOvertime"));
}

#[test]
fn free_verifies_call_keeps_its_method() {
    let scan = scan_file(
        Utf8Path::new("rules.test.ts"),
        Language::TypeScript,
        r#"function overtimeExamples() {
  verifies("rule_overtime", "examples");
}"#,
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].verification, Some(Verification::Examples));
}

#[test]
fn declaration_silence_and_unknown_id_warnings_coexist() {
    let scan = scan_file(
        Utf8Path::new("mixed.test.ts"),
        Language::TypeScript,
        r#"const fixture = requirement.rule("expiry", {statement: "Expires after 30 days"});
const paysOvertime = rule("rule_not_in_graph", (hours: number) => hours > 38);"#,
    );

    let warnings = validate_bindings(std::slice::from_ref(&scan), ["rule_known".to_string()]);
    assert_eq!(warnings.len(), 1, "only the real binding warns");
    assert_eq!(warnings[0].rule_id, "rule_not_in_graph");
}

#[test]
fn java_qualified_helper_keeps_binding_through_the_shared_parser() {
    let scan = scan_file(
        Utf8Path::new("PayrollRules.java"),
        Language::Java,
        r#"private static final IntPredicate PAYS_OVERTIME =
    ProvenanceRules.rule("rule_overtime", hours -> hours > 38);"#,
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("PAYS_OVERTIME"));
}
