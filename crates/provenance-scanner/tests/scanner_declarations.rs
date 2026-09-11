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
fn namespace_imported_helpers_bind_their_implementation_and_verification() {
    // The exact binding lines of the packed-consumer fixture this
    // repository ships to npm consumers.
    let scan = scan_file(
        Utf8Path::new("rule-bindings.ts"),
        Language::TypeScript,
        r#"import * as ruleBindings from "@quality-sh/provenance/rules";

const implementation = (hours: number): boolean => hours > 38;
const paysOvertime: (hours: number) => boolean = ruleBindings.rule("rule_packed_consumer_overtime", implementation);

function overtimeExamples(): void {
  ruleBindings.verifies("rule_packed_consumer_overtime", "examples");
}"#,
    );

    assert_eq!(scan.bindings.len(), 2);
    assert_eq!(scan.bindings[0].rule_id, "rule_packed_consumer_overtime");
    assert_eq!(scan.bindings[0].verification, None);
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("paysOvertime"));
    assert_eq!(scan.bindings[1].verification, Some(Verification::Examples));
    assert_eq!(
        scan.bindings[1].item_name.as_deref(),
        Some("overtimeExamples")
    );
}

#[test]
fn the_shipped_packed_consumer_fixture_scans_both_sites() {
    let fixture = Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/provenance/test/fixtures/packed-consumer/rule-bindings.ts.fixture");
    let source = std::fs::read_to_string(&fixture)
        .expect("packed consumer fixture ships beside the scanner crate");
    let scan = scan_file(
        Utf8Path::new("rule-bindings.ts"),
        Language::TypeScript,
        &source,
    );

    let sites: Vec<(String, Option<&str>, Option<Verification>)> = scan
        .bindings
        .iter()
        .map(|binding| {
            (
                binding.rule_id.clone(),
                binding.item_name.as_deref(),
                binding.verification,
            )
        })
        .collect();
    assert_eq!(
        sites,
        vec![
            (
                "rule_packed_consumer_overtime".to_string(),
                Some("paysOvertime"),
                None
            ),
            (
                "rule_packed_consumer_overtime".to_string(),
                Some("overtimeExamples"),
                Some(Verification::Examples)
            ),
        ]
    );
}

#[test]
fn tracked_sdk_declaration_shapes_never_bind() {
    for source in [
        r#"const expiry = sharing.rule("expiry", {
  statement: "Share links expire within 30 days",
});"#,
        r#"expiry: sharing.rule("expiry", { statement: "Share links expire" });"#,
        r#"const shareExpiry = sharing.rule("expiry").statement("Share links expire");"#,
        r#"() => escapedRequirement?.rule("late", {})"#,
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
