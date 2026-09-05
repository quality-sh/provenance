//! Which active Rules the scan calls unimplemented.

use super::unimplemented_rule_warnings;
use camino::Utf8PathBuf;
use provenance_core::coverage::EvidenceAnchor;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{Rule, RuleSeverity, RuleStatus, ScopeId, StableId};
use provenance_scanner::{AttributeBinding, FileScan, Language, Verification};

fn rule(id: &str, status: RuleStatus) -> Rule {
    Rule {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        declared_by: None,
        declaration_address: None,
        retired: false,
        name: None,
        description: None,
        statement: "Claims must be grouped by participant".to_string(),
        status,
        severity: RuleSeverity::High,
        source_document: None,
        source_section: None,
        requirement_ids: Vec::new(),
        resolution_ids: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

fn scan_with_binding(rule_id: &str, verification: Option<Verification>) -> FileScan {
    FileScan {
        file_path: Utf8PathBuf::from("src/lib.rs"),
        language: Language::Rust,
        annotations: Vec::new(),
        bindings: vec![AttributeBinding {
            file_path: Utf8PathBuf::from("src/lib.rs"),
            line: 1,
            item_name: Some("implementation_or_test".to_string()),
            rule_id: rule_id.to_string(),
            verification,
            anchor: EvidenceAnchor::new(Some("implementation_or_test".to_string()), "#[rule]"),
        }],
        warnings: Vec::new(),
    }
}

fn scan_comment(source: &str) -> FileScan {
    provenance_scanner::scan_file(camino::Utf8Path::new("src/lib.rs"), Language::Rust, source)
}

#[test]
fn active_rule_without_an_implementation_warns() {
    let warnings = unimplemented_rule_warnings(&[rule("rule_foo", RuleStatus::Active)], &[], &[]);

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].rule_id, "rule_foo");
    assert_eq!(warnings[0].file_path, None);
    assert_eq!(warnings[0].line, None);
    assert!(warnings[0].message.contains("has no implementation"));
}

#[test]
fn scanned_primary_implementation_binding_satisfies_the_finding() {
    let active = rule("rule_foo", RuleStatus::Active);
    let scan = scan_with_binding("rule_foo", None);

    assert!(unimplemented_rule_warnings(&[active], &[scan], &[]).is_empty());
}

#[test]
fn verification_without_an_implementation_only_warns_as_unimplemented() {
    let active = rule("rule_foo", RuleStatus::Active);
    let scan = scan_with_binding("rule_foo", Some(Verification::Examples));

    let warnings = unimplemented_rule_warnings(&[active], &[scan], &[]);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("has no implementation"));
    assert!(!warnings[0].message.contains("has no #[rule]"));
}

#[test]
fn comment_implementation_satisfies_the_finding() {
    let active = rule("rule_foo", RuleStatus::Active);
    let scan = scan_comment("// @provenance rule: rule_foo\nfn implementation() {}");

    assert!(unimplemented_rule_warnings(&[active], &[scan], &[]).is_empty());
}

#[test]
fn comment_verification_does_not_count_as_an_implementation() {
    let active = rule("rule_foo", RuleStatus::Active);
    let scan = scan_comment(
        "// @provenance rule: rule_foo\n// @provenance verification: examples\nfn verifies_it() {}",
    );

    assert_eq!(
        unimplemented_rule_warnings(&[active], &[scan], &[]).len(),
        1
    );
}

#[test]
fn source_citations_do_not_count_as_an_implementation() {
    let active = Rule {
        source_document: Some("src/lib.rs".to_string()),
        source_section: Some("cited_section".to_string()),
        ..rule("rule_foo", RuleStatus::Active)
    };

    assert_eq!(unimplemented_rule_warnings(&[active], &[], &[]).len(), 1);
}

#[test]
fn draft_and_deprecated_rules_do_not_warn() {
    let draft = rule("rule_draft", RuleStatus::Draft);
    let deprecated = rule("rule_deprecated", RuleStatus::Deprecated);

    assert!(unimplemented_rule_warnings(&[draft, deprecated], &[], &[]).is_empty());
}

#[test]
fn retired_active_rule_does_not_warn_about_missing_implementation() {
    let retired = Rule {
        retired: true,
        ..rule("rule_retired", RuleStatus::Active)
    };

    assert!(unimplemented_rule_warnings(&[retired], &[], &[]).is_empty());
}
