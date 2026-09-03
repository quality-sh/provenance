use super::*;

#[test]
fn scans_rust_annotation_with_location() {
    let scan = scan_file(
        Utf8Path::new("payroll.rs"),
        Language::Rust,
        "// @provenance rule: SCHADS-PAY-001\nfn pays_overtime() {}",
    );

    assert_eq!(scan.annotations[0].line, 1);
    assert_eq!(
        scan.annotations[0].function_name.as_deref(),
        Some("pays_overtime")
    );
}

#[test]
fn typescript_comment_annotation_uses_the_exported_function_name() {
    let scan = scan_file(
        Utf8Path::new("runtime.ts"),
        Language::TypeScript,
        "// @provenance rule: rule_start\nexport function startWorkflow(): void {}",
    );

    assert_eq!(
        scan.annotations[0].function_name.as_deref(),
        Some("startWorkflow")
    );
}

#[test]
fn scans_rule_attribute_with_item_name() {
    let scan = scan_file(
        Utf8Path::new("relations.rs"),
        Language::Rust,
        "#[rule(\"rule_prov_relation_vocabulary_closed\")]\npub fn declared_relations() {}",
    );

    assert_eq!(
        scan.bindings,
        vec![AttributeBinding {
            file_path: Utf8Path::new("relations.rs").to_path_buf(),
            line: 1,
            item_name: Some("declared_relations".to_string()),
            rule_id: "rule_prov_relation_vocabulary_closed".to_string(),
            verification: None,
            anchor: EvidenceAnchor::new(
                Some("declared_relations".to_string()),
                "#[rule(\"rule_prov_relation_vocabulary_closed\")]",
            ),
        }]
    );
}

#[test]
fn scans_rule_attribute_on_a_type_as_an_implementation() {
    let source = "#[rule(\"rule_valid_token\")]\npub struct ValidToken(String);";
    let scan = scan_file(Utf8Path::new("tokens.rs"), Language::Rust, source);
    let binding = &scan.bindings[0];
    assert_eq!(binding.verification, None);
    assert_eq!(binding.item_name.as_deref(), Some("ValidToken"));
}

#[test]
fn scans_verifies_attribute_past_test_attribute() {
    let scan = scan_file(
        Utf8Path::new("relations.rs"),
        Language::Rust,
        "#[test]\n#[verifies(\"rule_prov_relation_vocabulary_closed\", exhaustion)]\nfn every_owner_kind_appears_once_in_the_declared_tables() {}",
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(
        scan.bindings[0].verification,
        Some(Verification::Exhaustion)
    );
    assert_eq!(
        scan.bindings[0].item_name.as_deref(),
        Some("every_owner_kind_appears_once_in_the_declared_tables")
    );
}

#[test]
fn scans_construction_verifies_attribute_on_a_type() {
    let scan = scan_file(
        Utf8Path::new("tokens.rs"),
        Language::Rust,
        "#[verifies(\"rule_redacted_display\", construction)]\npub struct RedactedToken(String);",
    );

    assert_eq!(
        scan.bindings[0].verification,
        Some(Verification::Construction)
    );
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("RedactedToken"));
}

#[test]
fn scans_legacy_statesman_annotation_with_location() {
    let scan = scan_file(
        Utf8Path::new("payroll.rs"),
        Language::Rust,
        "// @statesman rule: SCHADS-PAY-001\nfn pays_overtime() {}",
    );

    assert_eq!(scan.annotations.len(), 1);
    assert_eq!(scan.annotations[0].line, 1);
    assert_eq!(scan.warnings.len(), 1);
    assert!(scan.warnings[0].message.contains("legacy marker"));
}
