//! Which bindings to deprecated or archived Rules are current, and which
//! findings the lifecycle policy governs.

use super::lifecycle::inactive_rule_binding_warnings;
use crate::handlers::coverage::retired::stale_rule_warnings;
use camino::Utf8PathBuf;
use provenance_core::coverage::EvidenceAnchor;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    ImplementationBinding, Rule, RuleStatus, ScopeId, StableId, VerificationBinding,
    VerificationMethod,
};
use provenance_scanner::{
    Annotation, AnnotationLocation, AttributeBinding, CoverageLevel, FileScan, Language,
    Verification,
};

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
        severity: provenance_core::RuleSeverity::Medium,
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

fn scan_with_annotation_marker(rule_id: &str) -> FileScan {
    let mut scan = empty_scan();
    scan.annotations = vec![AnnotationLocation {
        file_path: Utf8PathBuf::from("src/lib.rs"),
        line: 4,
        function_name: Some("carries_it".to_string()),
        anchor: EvidenceAnchor::new(Some("carries_it".to_string()), "// @provenance"),
        annotation: Annotation {
            rule: rule_id.to_string(),
            name: None,
            description: None,
            tags: Vec::new(),
            coverage: CoverageLevel::Full,
            confidence: 1.0,
            intent: None,
            verification: None,
        },
    }];
    scan
}

fn scan_with_attribute_binding(rule_id: &str, verification: Option<Verification>) -> FileScan {
    let mut scan = empty_scan();
    scan.bindings = vec![AttributeBinding {
        file_path: Utf8PathBuf::from("src/lib.rs"),
        line: 4,
        item_name: Some("carries_it".to_string()),
        rule_id: rule_id.to_string(),
        verification,
        anchor: EvidenceAnchor::new(Some("carries_it".to_string()), "#[rule]"),
    }];
    scan
}

fn typed_implementation(rule_id: &str, retired: bool) -> ImplementationBinding {
    ImplementationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new("implementation_binding_lifecycle").unwrap(),
        rule_id: StableId::new(rule_id).unwrap(),
        declared_by: "spec://test/owner".to_string(),
        retired,
        file: "src/billing.rs".into(),
        symbol: "bills_overtime".to_string(),
    }
}

fn typed_verification(rule_id: &str, retired: bool) -> VerificationBinding {
    VerificationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new("verification_binding_lifecycle").unwrap(),
        rule_id: StableId::new(rule_id).unwrap(),
        key: "lifecycle-check".to_string(),
        method: VerificationMethod::Examples,
        declared_by: "ci://test".to_string(),
        retired,
        file: "tests/rule.test.ts".into(),
        symbol: Some("rule holds".to_string()),
    }
}

#[test]
fn a_deprecated_rule_with_a_current_typed_implementation_binding_warns() {
    let deprecated = rule("rule_old_rate", RuleStatus::Deprecated);

    let warnings = inactive_rule_binding_warnings(
        &[deprecated],
        &[],
        &[typed_implementation("rule_old_rate", false)],
        &[],
    );

    assert_eq!(warnings.len(), 1, "{warnings:#?}");
    assert_eq!(warnings[0].rule_id, "rule_old_rate");
    assert!(warnings[0].message.contains("deprecated"), "{warnings:#?}");
    assert!(warnings[0].binding_finding);
}

#[test]
fn an_archived_rule_with_a_current_typed_verification_binding_warns() {
    let archived = rule("rule_old_gate", RuleStatus::Archived);

    let warnings = inactive_rule_binding_warnings(
        &[archived],
        &[],
        &[],
        &[typed_verification("rule_old_gate", false)],
    );

    assert_eq!(warnings.len(), 1, "{warnings:#?}");
    assert_eq!(warnings[0].rule_id, "rule_old_gate");
    assert!(warnings[0].message.contains("archived"), "{warnings:#?}");
    assert!(warnings[0].binding_finding);
}

#[test]
fn a_deprecated_rule_without_current_bindings_does_not_warn() {
    let deprecated = rule("rule_old_rate", RuleStatus::Deprecated);
    let archived = rule("rule_old_gate", RuleStatus::Archived);

    let warnings = inactive_rule_binding_warnings(&[deprecated, archived], &[], &[], &[]);

    assert!(warnings.is_empty(), "{warnings:#?}");
}

#[test]
fn retired_typed_bindings_are_not_current_evidence_for_the_finding() {
    let deprecated = rule("rule_old_rate", RuleStatus::Deprecated);

    let warnings = inactive_rule_binding_warnings(
        &[deprecated],
        &[],
        &[typed_implementation("rule_old_rate", true)],
        &[typed_verification("rule_old_rate", true)],
    );

    assert!(warnings.is_empty(), "{warnings:#?}");
}

#[test]
fn scanned_markers_citing_inactive_rules_are_lifecycle_findings() {
    let deprecated = rule("rule_old_rate", RuleStatus::Deprecated);
    let archived = rule("rule_old_gate", RuleStatus::Archived);
    let marker = scan_with_annotation_marker("rule_old_rate");

    let warnings = inactive_rule_binding_warnings(
        &[deprecated, archived],
        std::slice::from_ref(&marker),
        &[],
        &[],
    );

    assert_eq!(warnings.len(), 1, "{warnings:#?}");
    assert_eq!(warnings[0].rule_id, "rule_old_rate");
    assert_eq!(
        warnings[0].file_path.as_deref(),
        Some(Utf8PathBuf::from("src/lib.rs").as_path())
    );
    assert_eq!(warnings[0].line, Some(4));
    assert!(warnings[0].binding_finding);
}

#[test]
fn scanned_attribute_bindings_citing_inactive_rules_are_lifecycle_findings() {
    let archived = rule("rule_old_gate", RuleStatus::Archived);
    let binding = scan_with_attribute_binding("rule_old_gate", None);

    let warnings =
        inactive_rule_binding_warnings(&[archived], std::slice::from_ref(&binding), &[], &[]);

    assert_eq!(warnings.len(), 1, "{warnings:#?}");
    assert!(warnings[0].message.contains("archived"), "{warnings:#?}");
    assert!(warnings[0].binding_finding);
}

#[test]
fn retired_rules_stay_with_the_separate_retired_record_check() {
    let retired_deprecated = Rule {
        retired: true,
        ..rule("rule_replaced", RuleStatus::Deprecated)
    };
    let marker = scan_with_annotation_marker("rule_replaced");

    let lifecycle = inactive_rule_binding_warnings(
        std::slice::from_ref(&retired_deprecated),
        std::slice::from_ref(&marker),
        &[typed_implementation("rule_replaced", false)],
        &[],
    );
    let retired = stale_rule_warnings(
        std::slice::from_ref(&retired_deprecated),
        std::slice::from_ref(&marker),
    );

    assert!(lifecycle.is_empty(), "{lifecycle:#?}");
    assert_eq!(retired.len(), 1, "{retired:#?}");
    assert!(
        !retired[0].binding_finding,
        "the retired check stays separate"
    );
}

#[test]
fn active_and_draft_rules_never_appear_in_lifecycle_findings() {
    let active = rule("rule_now", RuleStatus::Active);
    let draft = rule("rule_next", RuleStatus::Draft);

    let warnings = inactive_rule_binding_warnings(
        &[active, draft],
        &[],
        &[
            typed_implementation("rule_now", false),
            typed_implementation("rule_next", false),
        ],
        &[],
    );

    assert!(warnings.is_empty(), "{warnings:#?}");
}
