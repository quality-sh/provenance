use provenance_core::{
    Requirement, RequirementStatus, Rule, RuleSeverity, RuleStatus, SchemaVersion, ScopeId,
    StableId,
};
use provenance_macros::verifies;
use provenance_ste100::{FindingKind, RuleNumber, Span, Standard, StandardIssue, ANALYZER_VERSION};
use provenance_store::statement_analysis::{analyze_changed_statements, StatementRecordKind};

fn requirement(id: &str, statement: &str) -> Requirement {
    Requirement {
        schema_version: SchemaVersion(1),
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        declared_by: None,
        declaration_address: None,
        statement: statement.to_owned(),
        description: None,
        fog: None,
        status: RequirementStatus::Active,
        retired: false,
        domain_id: None,
        source_refs: Vec::new(),
        refines: None,
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn rule(id: &str, statement: &str) -> Rule {
    Rule {
        schema_version: SchemaVersion(1),
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        declared_by: None,
        declaration_address: None,
        name: None,
        description: None,
        statement: statement.to_owned(),
        status: RuleStatus::Active,
        retired: false,
        severity: RuleSeverity::High,
        source_document: None,
        source_section: None,
        requirement_ids: Vec::new(),
        resolution_ids: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

fn requirement_in(scope: &str, id: &str, statement: &str) -> Requirement {
    let mut record = requirement(id, statement);
    record.scope_id = ScopeId::new(scope).unwrap();
    record
}

#[test]
#[verifies("rule_ste_changed_statement_selection", examples)]
fn comparison_checks_additions_and_statement_byte_changes_only() {
    let old_requirements = vec![
        requirement("req_changed", "Old; invalid"),
        requirement("req_legacy", "Legacy; invalid"),
    ];
    let old_rules = vec![rule("rule_changed", "Old; invalid")];
    let new_requirements = vec![
        requirement("req_added", "Added; invalid"),
        requirement("req_changed", "New; invalid"),
        requirement("req_legacy", "Legacy; invalid"),
    ];
    let new_rules = vec![
        rule("rule_added", "Added; invalid"),
        rule("rule_changed", "New; invalid"),
    ];

    let diagnostics = analyze_changed_statements(
        &old_requirements,
        &old_rules,
        &new_requirements,
        &new_rules,
        None,
    );

    let identities = diagnostics
        .iter()
        .map(|item| (item.resource_kind, item.id.as_str(), item.span.start))
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        vec![
            (StatementRecordKind::Requirement, "req_added", 5),
            (StatementRecordKind::Requirement, "req_changed", 3),
            (StatementRecordKind::Rule, "rule_added", 5),
            (StatementRecordKind::Rule, "rule_changed", 3),
        ]
    );
}

#[test]
#[verifies("rule_ste_changed_statement_selection", examples)]
fn diagnostics_preserve_ste_report_fields_utf8_spans_and_stable_order() {
    let candidate_requirements = vec![requirement("req_z", "é;;"), requirement("req_a", "A;")];
    let candidate_rules = vec![rule("rule_a", "雪;")];

    let diagnostics =
        analyze_changed_statements(&[], &[], &candidate_requirements, &candidate_rules, None);

    let ordered = diagnostics
        .iter()
        .map(|item| (item.resource_kind, item.id.as_str(), item.span))
        .collect::<Vec<_>>();
    assert_eq!(
        ordered,
        vec![
            (
                StatementRecordKind::Requirement,
                "req_a",
                Span { start: 1, end: 2 }
            ),
            (
                StatementRecordKind::Requirement,
                "req_z",
                Span { start: 2, end: 3 }
            ),
            (
                StatementRecordKind::Requirement,
                "req_z",
                Span { start: 3, end: 4 }
            ),
            (
                StatementRecordKind::Rule,
                "rule_a",
                Span { start: 3, end: 4 }
            ),
        ]
    );
    for diagnostic in diagnostics {
        assert_eq!(diagnostic.field, "statement");
        assert_eq!(diagnostic.standard, Standard::AsdSte100);
        assert_eq!(diagnostic.issue, StandardIssue::Nine);
        assert_eq!(diagnostic.analyzer_version, ANALYZER_VERSION);
        assert_eq!(diagnostic.rule, RuleNumber::EightOne);
        assert_eq!(diagnostic.disposition, FindingKind::Violation);
        assert_eq!(
            diagnostic.message,
            "Do not use semicolons in descriptive text."
        );
    }
}

#[test]
#[verifies("rule_ste_changed_statement_selection", examples)]
fn non_statement_changes_and_clean_changed_statements_have_no_findings() {
    let base = requirement("req_same", "Legacy; invalid");
    let mut metadata_change = base.clone();
    metadata_change.description = Some("Unrelated metadata".to_owned());
    let clean = requirement("req_clean", "Clean statement");

    assert!(
        analyze_changed_statements(&[base], &[], &[metadata_change, clean], &[], None).is_empty()
    );
}

#[test]
#[verifies("rule_ste_changed_statement_selection", examples)]
fn scope_is_part_of_identity_and_diagnostic_order() {
    let base = vec![requirement_in("beta", "req_same", "Legacy; invalid")];
    let candidates = vec![
        requirement_in("beta", "req_same", "Legacy; invalid"),
        requirement_in("zeta", "req_same", "Z; changed"),
        requirement_in("alpha", "req_same", "A; changed"),
    ];

    let diagnostics = analyze_changed_statements(&base, &[], &candidates, &[], None);

    assert_eq!(
        diagnostics
            .iter()
            .map(|item| (item.scope_id.as_str(), item.id.as_str()))
            .collect::<Vec<_>>(),
        vec![("alpha", "req_same"), ("zeta", "req_same")]
    );
}
