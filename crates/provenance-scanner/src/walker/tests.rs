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
fn scans_verifies_attribute_wrapped_across_lines() {
    let source = "#[verifies(\n    \"rule_this_is_a_long_id_to_reproduce_a_wrapped_attribute_after_rustfmt\",\n    examples\n)]\nfn wrapped() {}";
    let scan = scan_file(Utf8Path::new("wrapped.rs"), Language::Rust, source);

    assert_eq!(scan.bindings.len(), 1, "{source}");
    assert_eq!(scan.bindings[0].line, 1);
    assert_eq!(
        scan.bindings[0].rule_id,
        "rule_this_is_a_long_id_to_reproduce_a_wrapped_attribute_after_rustfmt"
    );
    assert_eq!(scan.bindings[0].verification, Some(Verification::Examples));
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("wrapped"));
}

#[test]
fn scans_rule_attribute_wrapped_across_lines() {
    let source = "#[rule(\n    \"rule_wrapped_rule\"\n)]\nfn bound() {}";
    let scan = scan_file(Utf8Path::new("wrapped.rs"), Language::Rust, source);

    assert_eq!(scan.bindings.len(), 1, "{source}");
    assert_eq!(scan.bindings[0].rule_id, "rule_wrapped_rule");
    assert_eq!(scan.bindings[0].verification, None);
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("bound"));
}

#[test]
fn scans_qualified_rule_attribute() {
    let source =
        "#[provenance_macros::rule(\"rule_init_managed_paths_stay_in_repository\")]\nfn apply() {}";
    let scan = scan_file(Utf8Path::new("qualified.rs"), Language::Rust, source);

    assert_eq!(scan.bindings.len(), 1, "{source}");
    assert_eq!(
        scan.bindings[0].rule_id,
        "rule_init_managed_paths_stay_in_repository"
    );
    assert_eq!(scan.bindings[0].verification, None);
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("apply"));
}

#[test]
fn scans_qualified_verifies_attribute_wrapped_across_lines() {
    let source = "#[provenance_macros::verifies(\n    \"rule_init_apply_rolls_back_owned_changes\",\n    examples\n)]\nfn rollback_examples() {}";
    let scan = scan_file(Utf8Path::new("qualified.rs"), Language::Rust, source);

    assert_eq!(scan.bindings.len(), 1, "{source}");
    assert_eq!(
        scan.bindings[0].rule_id,
        "rule_init_apply_rolls_back_owned_changes"
    );
    assert_eq!(scan.bindings[0].verification, Some(Verification::Examples));
    assert_eq!(
        scan.bindings[0].item_name.as_deref(),
        Some("rollback_examples")
    );
}

#[test]
fn scans_wrapped_verifies_attribute_past_a_test_attribute() {
    let source = "#[test]\n#[verifies(\n    \"rule_wrapped_after_test\",\n    property\n)]\nfn generated_inputs_are_checked() {}";
    let scan = scan_file(Utf8Path::new("wrapped.rs"), Language::Rust, source);

    assert_eq!(scan.bindings.len(), 1, "{source}");
    assert_eq!(scan.bindings[0].rule_id, "rule_wrapped_after_test");
    assert_eq!(scan.bindings[0].verification, Some(Verification::Property));
    assert_eq!(
        scan.bindings[0].item_name.as_deref(),
        Some("generated_inputs_are_checked")
    );
}

#[test]
fn wrapped_attribute_inside_a_raw_string_never_binds() {
    let source = "const FIXTURE: &str = r#\"\n#[verifies(\n    \"rule_not_bound\",\n    examples\n)]\nfn fake() {}\n\"#;";
    let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, source);

    assert!(scan.bindings.is_empty(), "{source}");
}

#[test]
fn qualified_attribute_inside_a_block_comment_never_binds() {
    let source = "/*\n#[provenance_macros::rule(\"rule_not_bound\")]\n*/\nfn after() {}";
    let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, source);

    assert!(scan.bindings.is_empty(), "{source}");
}

#[test]
fn unbalanced_wrapped_attribute_never_binds_or_hangs() {
    let source = "#[verifies(\n    \"rule_unbalanced\",\n    examples\nfn orphan() {}";
    let scan = scan_file(Utf8Path::new("broken.rs"), Language::Rust, source);

    assert!(scan.bindings.is_empty(), "{source}");
}

#[test]
fn scans_rule_attribute_on_a_trait() {
    let scan = scan_file(
        Utf8Path::new("projections.rs"),
        Language::Rust,
        "#[rule(\"rule_trait_triage\")]\npub trait ProjectionRow {}",
    );

    assert_eq!(scan.bindings[0].verification, None);
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("ProjectionRow"));
}

#[test]
fn trait_binding_names_the_trait_not_a_later_item() {
    let source = "#[rule(\"rule_trait_methods\")]\npub trait GenericStore {\n    fn put(&mut self, value: u8);\n    fn get(&self) -> Option<u8>;\n}\n\nstruct Unrelated;";
    let scan = scan_file(Utf8Path::new("stores.rs"), Language::Rust, source);

    assert_eq!(scan.bindings.len(), 1, "{source}");
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("GenericStore"));
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

#[test]
fn probe_indented_qualified() {
    let source = "mod tests {\n    #[test]\n    #[provenance_macros::verifies(\"rule_x\", examples)]\n    fn probe_case() {}\n}\n";
    let scan = scan_file(Utf8Path::new("probe.rs"), Language::Rust, source);
    eprintln!("PROBE: {:?}", scan.bindings);
}
