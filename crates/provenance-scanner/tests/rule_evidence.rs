use camino::Utf8PathBuf;
use provenance_core::coverage::EvidenceAnchor;
use provenance_core::{
    ImplementationBinding, Rule, RuleSeverity, RuleStatus, ScopeId, StableId, VerificationBinding,
    VerificationMethod, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::verifies;
use provenance_scanner::{
    binding_findings_fail, derive_rule_evidence_facts, AttributeBinding, BindingFindingSeverity,
    FileScan, InactiveBindingOrigin, Language, RuleEvidenceCompleteness, Verification,
};

fn rule(id: &str, status: RuleStatus) -> Rule {
    Rule {
        created: None,
        updated: None,
        archived_in_commit: None,
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        declared_by: None,
        declaration_address: None,
        name: None,
        description: None,
        statement: "The system groups claims by participant".to_string(),
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

fn scanned_binding(rule_id: &str, verification: Option<Verification>) -> FileScan {
    FileScan {
        bindings: vec![AttributeBinding {
            file_path: Utf8PathBuf::from("src/lib.rs"),
            line: 4,
            item_name: Some("checks_claims".to_string()),
            rule_id: rule_id.to_string(),
            verification,
            anchor: EvidenceAnchor::new(Some("checks_claims".to_string()), "#[rule]"),
        }],
        ..empty_scan()
    }
}

fn scanned_comment(source: &str) -> FileScan {
    provenance_scanner::scan_file(
        Utf8PathBuf::from("src/lib.rs").as_path(),
        Language::Rust,
        source,
    )
}

fn typed_implementation(rule_id: &str) -> ImplementationBinding {
    ImplementationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new("implementation_binding_claims").unwrap(),
        rule_id: StableId::new(rule_id).unwrap(),
        declared_by: "spec://test/owner".to_string(),
        file: "src/claims.rs".into(),
        symbol: "groups_claims".to_string(),
    }
}

fn typed_verification(rule_id: &str) -> VerificationBinding {
    VerificationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new("verification_binding_claims").unwrap(),
        rule_id: StableId::new(rule_id).unwrap(),
        key: "claims-check".to_string(),
        method: VerificationMethod::Examples,
        declared_by: "ci://test".to_string(),
        file: "tests/claims.rs".into(),
        symbol: Some("groups_claims".to_string()),
    }
}

#[test]
fn incomplete_evidence_withholds_absence_facts() {
    let facts = derive_rule_evidence_facts(
        &[rule("rule_claims", RuleStatus::Active)],
        &[],
        &[],
        &[],
        RuleEvidenceCompleteness::Incomplete,
    );

    assert!(facts.unimplemented.is_empty());
    assert!(facts.unverified.is_empty());
}

#[test]
#[verifies("rule_active_rule_requires_verification", examples)]
#[verifies("rule_active_rule_reports_missing_implementation", examples)]
fn complete_evidence_reports_each_active_rule_absence() {
    let facts = derive_rule_evidence_facts(
        &[rule("rule_claims", RuleStatus::Active)],
        &[],
        &[],
        &[],
        RuleEvidenceCompleteness::Complete,
    );

    assert_eq!(facts.unimplemented, ["rule_claims"]);
    assert_eq!(facts.unverified, ["rule_claims"]);
    assert_eq!(facts.governed_finding_count(), 1);

    let verified = scanned_binding("rule_claims", Some(Verification::Examples));
    let implementation_only_absence = derive_rule_evidence_facts(
        &[rule("rule_claims", RuleStatus::Active)],
        &[verified],
        &[],
        &[],
        RuleEvidenceCompleteness::Complete,
    );

    assert_eq!(implementation_only_absence.unimplemented, ["rule_claims"]);
    assert!(implementation_only_absence.unverified.is_empty());
    assert_eq!(implementation_only_absence.governed_finding_count(), 0);
    assert!(!binding_findings_fail(
        BindingFindingSeverity::Error,
        implementation_only_absence.governed_finding_count()
    ));
}

#[test]
fn portable_comment_implementation_satisfies_implementation() {
    let scan = scanned_comment("// @provenance rule: rule_claims\nfn groups_claims() {}\n");

    let facts = derive_rule_evidence_facts(
        &[rule("rule_claims", RuleStatus::Active)],
        &[scan],
        &[],
        &[],
        RuleEvidenceCompleteness::Complete,
    );

    assert!(facts.unimplemented.is_empty(), "{facts:#?}");
    assert_eq!(facts.unverified, ["rule_claims"]);
}

#[test]
fn portable_comment_verification_satisfies_only_verification() {
    let scan = scanned_comment(
        "// @provenance rule: rule_claims\n// @provenance verification: examples\nfn checks_claims() {}\n",
    );

    let facts = derive_rule_evidence_facts(
        &[rule("rule_claims", RuleStatus::Active)],
        &[scan],
        &[],
        &[],
        RuleEvidenceCompleteness::Complete,
    );

    assert_eq!(facts.unimplemented, ["rule_claims"]);
    assert!(facts.unverified.is_empty(), "{facts:#?}");
}

#[test]
fn inactive_rule_with_portable_comment_marker_reports_current_binding() {
    let scan = scanned_comment("// @provenance rule: rule_old\nfn old_claims() {}\n");

    let facts = derive_rule_evidence_facts(
        &[rule("rule_old", RuleStatus::Deprecated)],
        &[scan],
        &[],
        &[],
        RuleEvidenceCompleteness::Complete,
    );

    assert_eq!(facts.inactive_current.len(), 1, "{facts:#?}");
    assert_eq!(facts.inactive_current[0].rule_id, "rule_old");
    assert_eq!(
        facts.inactive_current[0].origin,
        InactiveBindingOrigin::Scanned
    );
}

#[test]
fn scanned_and_typed_bindings_form_one_evidence_union() {
    let scanned = rule("rule_scanned", RuleStatus::Active);
    let typed = rule("rule_typed", RuleStatus::Active);
    let implementation = scanned_binding("rule_scanned", None);
    let verification = scanned_binding("rule_scanned", Some(Verification::Examples));

    let facts = derive_rule_evidence_facts(
        &[scanned, typed],
        &[implementation, verification],
        &[typed_implementation("rule_typed")],
        &[typed_verification("rule_typed")],
        RuleEvidenceCompleteness::Complete,
    );

    assert!(facts.unimplemented.is_empty(), "{facts:#?}");
    assert!(facts.unverified.is_empty(), "{facts:#?}");
}

#[test]
fn verification_does_not_count_as_an_implementation() {
    let scan = scanned_binding("rule_claims", Some(Verification::Examples));

    let facts = derive_rule_evidence_facts(
        &[rule("rule_claims", RuleStatus::Active)],
        &[scan],
        &[],
        &[],
        RuleEvidenceCompleteness::Complete,
    );

    assert_eq!(facts.unimplemented, ["rule_claims"]);
    assert!(facts.unverified.is_empty());
}

#[test]
fn non_active_rules_do_not_produce_absence_facts() {
    let facts = derive_rule_evidence_facts(
        &[
            rule("rule_draft", RuleStatus::Draft),
            rule("rule_old", RuleStatus::Deprecated),
        ],
        &[],
        &[],
        &[],
        RuleEvidenceCompleteness::Complete,
    );

    assert!(facts.unimplemented.is_empty());
    assert!(facts.unverified.is_empty());
}

#[test]
#[verifies("rule_inactive_rules_have_no_current_bindings", examples)]
fn inactive_rules_report_scanned_and_typed_current_bindings() {
    let scan = scanned_binding("rule_old", Some(Verification::Property));

    let facts = derive_rule_evidence_facts(
        &[rule("rule_old", RuleStatus::Deprecated)],
        &[scan],
        &[typed_implementation("rule_old")],
        &[typed_verification("rule_old")],
        RuleEvidenceCompleteness::Incomplete,
    );

    assert_eq!(facts.inactive_current.len(), 3, "{facts:#?}");
    assert_eq!(facts.inactive_current[0].rule_id, "rule_old");
    assert_eq!(facts.inactive_current[0].status, RuleStatus::Deprecated);
    assert_eq!(
        facts.inactive_current[0].origin,
        InactiveBindingOrigin::Scanned
    );
    assert_eq!(
        facts.inactive_current[1].origin,
        InactiveBindingOrigin::TypedImplementation
    );
    assert_eq!(
        facts.inactive_current[2].origin,
        InactiveBindingOrigin::TypedVerification
    );
}

#[test]
#[verifies("rule_binding_finding_uses_configured_severity", examples)]
fn binding_finding_policy_fails_only_for_errors_with_governed_findings() {
    assert!(binding_findings_fail(BindingFindingSeverity::Error, 1));
    assert!(!binding_findings_fail(BindingFindingSeverity::Error, 0));
    assert!(!binding_findings_fail(BindingFindingSeverity::Warning, 1));
}
