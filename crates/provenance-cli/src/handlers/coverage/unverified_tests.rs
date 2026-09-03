//! Which active rules the scan calls unverified, and what it says about
//! them.

use super::unverified_rule_warnings;
use camino::Utf8PathBuf;
use provenance_core::coverage::EvidenceAnchor;
use provenance_core::{
    Rule, RuleSeverity, RuleStatus, SchemaVersion, ScopeId, StableId, VerificationBinding,
    VerificationMethod,
};
use provenance_scanner::{
    Annotation, AnnotationLocation, AttributeBinding, CoverageLevel, FileScan, Language,
    Verification,
};

fn rule(id: &str, status: RuleStatus) -> Rule {
    Rule {
        schema_version: SchemaVersion(1),
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

fn empty_scan() -> FileScan {
    FileScan {
        file_path: Utf8PathBuf::from("src/lib.rs"),
        language: Language::Rust,
        annotations: Vec::new(),
        bindings: Vec::new(),
        warnings: Vec::new(),
    }
}

fn scan_with_binding(rule_id: &str, verification: Option<Verification>) -> FileScan {
    FileScan {
        bindings: vec![AttributeBinding {
            file_path: Utf8PathBuf::from("src/lib.rs"),
            line: 1,
            item_name: Some("checks_it".to_string()),
            rule_id: rule_id.to_string(),
            verification,
            anchor: EvidenceAnchor::new(Some("checks_it".to_string()), "#[verifies]"),
        }],
        ..empty_scan()
    }
}

fn scan_with_annotation(rule_id: &str, verification: Option<Verification>) -> FileScan {
    FileScan {
        annotations: vec![AnnotationLocation {
            file_path: Utf8PathBuf::from("src/lib.rs"),
            line: 1,
            function_name: Some("checks_it".to_string()),
            anchor: EvidenceAnchor::new(Some("checks_it".to_string()), "// @provenance"),
            annotation: Annotation {
                rule: rule_id.to_string(),
                name: None,
                description: None,
                tags: Vec::new(),
                coverage: CoverageLevel::Full,
                confidence: 1.0,
                intent: None,
                verification,
            },
        }],
        ..empty_scan()
    }
}

fn typed_binding(rule_id: &str) -> VerificationBinding {
    VerificationBinding {
        schema_version: SchemaVersion(1),
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new("verification_binding_typed").unwrap(),
        rule_id: StableId::new(rule_id).unwrap(),
        key: "typed-check".to_string(),
        method: VerificationMethod::Examples,
        declared_by: "ci://typescript".to_string(),
        retired: false,
        file: "tests/rule.test.ts".into(),
        symbol: Some("rule holds".to_string()),
    }
}

#[test]
fn active_rule_with_no_verification_warns() {
    let active = rule("rule_foo", RuleStatus::Active);

    let warnings = unverified_rule_warnings(&[active], &[], &[]);

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].rule_id, "rule_foo");
    assert!(warnings[0].message.contains("has no verification"));
}

#[test]
fn retired_active_rule_does_not_warn_about_missing_verification() {
    let retired = Rule {
        retired: true,
        ..rule("rule_retired", RuleStatus::Active)
    };

    assert!(unverified_rule_warnings(&[retired], &[], &[]).is_empty());
}

/// The rule names a source document, and the warning still points at no
/// file: the document is where the rule came from, not where the missing
/// test would go.
#[test]
fn unverified_warning_names_no_file_or_line_even_when_the_rule_cites_a_document() {
    let active = Rule {
        source_document: Some("docs/awards/schads.md".to_string()),
        ..rule("rule_foo", RuleStatus::Active)
    };

    let warnings = unverified_rule_warnings(&[active], &[], &[]);

    assert_eq!(warnings[0].file_path, None);
    assert_eq!(warnings[0].line, None);
}

#[test]
fn active_rule_verified_via_attribute_binding_does_not_warn() {
    let active = rule("rule_foo", RuleStatus::Active);
    let scan = scan_with_binding("rule_foo", Some(Verification::Examples));

    let warnings = unverified_rule_warnings(&[active], std::slice::from_ref(&scan), &[]);

    assert!(warnings.is_empty());
}

#[test]
fn active_rule_verified_via_comment_annotation_does_not_warn() {
    let active = rule("rule_foo", RuleStatus::Active);
    let scan = scan_with_annotation("rule_foo", Some(Verification::Conformance));

    let warnings = unverified_rule_warnings(&[active], std::slice::from_ref(&scan), &[]);

    assert!(warnings.is_empty());
}

#[test]
fn comment_annotation_without_a_verification_key_does_not_count() {
    let active = rule("rule_foo", RuleStatus::Active);
    let scan = scan_with_annotation("rule_foo", None);

    let warnings = unverified_rule_warnings(&[active], std::slice::from_ref(&scan), &[]);

    assert_eq!(warnings.len(), 1);
}

#[test]
fn draft_and_deprecated_rules_never_warn() {
    let draft = rule("rule_draft", RuleStatus::Draft);
    let deprecated = rule("rule_deprecated", RuleStatus::Deprecated);

    let warnings = unverified_rule_warnings(&[draft, deprecated], &[], &[]);

    assert!(warnings.is_empty());
}

#[test]
fn verification_matches_from_either_channel() {
    let verified_by_comment = rule("rule_by_comment", RuleStatus::Active);
    let verified_by_attribute = rule("rule_by_attribute", RuleStatus::Active);
    // Both channels cite the rule id: attributes directly, comment
    // annotations through the marker's rule key.
    let binding_scan = scan_with_binding("rule_by_attribute", Some(Verification::Property));
    let annotation_scan = scan_with_annotation("rule_by_comment", Some(Verification::Property));

    let warnings = unverified_rule_warnings(
        &[verified_by_comment, verified_by_attribute],
        &[binding_scan, annotation_scan],
        &[],
    );

    assert!(warnings.is_empty());
}

#[test]
fn active_rule_unverified_alongside_verified_rules_only_warns_once() {
    let verified = rule("rule_verified", RuleStatus::Active);
    let unverified = rule("rule_unverified", RuleStatus::Active);
    let scan = scan_with_binding("rule_verified", Some(Verification::Exhaustion));

    let warnings =
        unverified_rule_warnings(&[verified, unverified], std::slice::from_ref(&scan), &[]);

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].rule_id, "rule_unverified");
}

#[test]
fn active_rule_with_a_typed_verification_binding_does_not_warn() {
    let active = rule("rule_foo", RuleStatus::Active);

    let warnings = unverified_rule_warnings(&[active], &[], &[typed_binding("rule_foo")]);

    assert!(warnings.is_empty());
}
