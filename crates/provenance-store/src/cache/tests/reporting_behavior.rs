use super::super::*;
use super::fixtures::*;
use crate::state_store::{
    CreateRequirementInput, CreateResolutionInput, CreateRuleInput, StateStore,
};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    NodeType, RequirementStatus, ResolutionStatus, RuleSeverity, RuleStatus, ScopeId,
};
use provenance_macros::verifies;

#[test]
fn impact_reports_hop_distance_and_direction() {
    let (_dir, layout, scope) = seeded_layout();
    let impact = analyze_impact(
        &layout,
        &scope,
        NodeType::Source,
        &sid("source_schads"),
        ImpactOptions {
            max_hops: 3,
            follow_indirect: true,
        },
    )
    .unwrap();
    let rule = impact
        .nodes
        .iter()
        .find(|node| node.id == "rule_schads_pay_001")
        .unwrap();
    assert_eq!(rule.hop_distance, 2);
    assert_eq!(rule.direction, ImpactDirection::Downstream);
}

#[test]
fn graph_evidence_lists_the_fixture_rule() {
    let (_dir, layout, scope) = seeded_layout();
    let evidence = graph_evidence(&layout, &scope, false).unwrap();
    assert!(evidence.rule_ids.contains("rule_schads_pay_001"));
}

#[test]
fn health_counts_rules_with_complete_traceability() {
    let (_dir, layout, scope) = seeded_layout();
    let health = coverage_health(&layout, &scope).unwrap();
    assert_eq!(health.rules.total, 1);
    assert_eq!(health.rules.with_complete_traceability, 1);
    assert_eq!(health.gaps.total, 0);
}

#[test]
fn retired_declarations_are_absent_from_active_health_and_gap_views() {
    let (_dir, layout, scope) = seeded_layout();
    retire_records(&layout, &scope);

    let health = coverage_health(&layout, &scope).unwrap();
    assert_eq!(health.requirements.total, 0);
    assert_eq!(health.rules.total, 0);
    assert_eq!(health.gaps.total, 0);
    assert!(graph_evidence(&layout, &scope, false)
        .unwrap()
        .rule_ids
        .is_empty());
    assert!(prime_context(&layout, &scope, false)
        .unwrap()
        .rules
        .is_empty());
}

/// Retiring a requirement that other records name must not manufacture
/// reference gaps. The loader drops a reference to a retired record the
/// way it drops the record itself, so `gaps` and `prime` agree with
/// `check`, whose unfiltered lists still resolve the reference.
#[test]
fn retiring_a_named_requirement_does_not_change_the_gap_report() {
    let (_dir, layout, scope) = seeded_layout();
    let store = StateStore::new(layout.clone());
    // A second live requirement keeps the source referenced after the
    // first one retires, so any delta in the report is the retirement.
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: sid("req_stays"),
            statement: "Still active".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: Some(sid("domain_payroll")),
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    attach_source(&store, &scope, "req_stays", "source_schads");

    let before = find_gaps(&layout, &scope).unwrap();

    // A hand edit stands in for sdk apply retiring an omitted requirement.
    let path = crate::shards::requirements_path(&layout, &scope);
    let contents = std::fs::read_to_string(&path).unwrap();
    let retired = contents
        .lines()
        .map(|line| {
            let mut record = serde_json::from_str::<serde_json::Value>(line).unwrap();
            if record["id"] == "req_schads_overtime" {
                record["retired"] = serde_json::Value::Bool(true);
            }
            serde_json::to_string(&record).unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, format!("{retired}\n")).unwrap();

    let after = find_gaps(&layout, &scope).unwrap();
    assert_eq!(after, before, "retirement manufactured gaps: {after:?}");
}

fn retire_records(layout: &ProvenanceLayout, scope: &ScopeId) {
    for path in [
        crate::shards::sources_path(layout, scope),
        crate::shards::requirements_path(layout, scope),
        crate::shards::rules_path(layout, scope),
    ] {
        let contents = std::fs::read_to_string(&path).unwrap();
        let retired = contents
            .lines()
            .map(|line| {
                let mut record = serde_json::from_str::<serde_json::Value>(line).unwrap();
                record["retired"] = serde_json::Value::Bool(true);
                serde_json::to_string(&record).unwrap()
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(path, format!("{retired}\n")).unwrap();
    }
}

/// Seeds a requirement, the decision that settles it, and a rule both are
/// recorded as producing. No source is attached to the requirement.
fn seed_unsourced_chain(layout: &ProvenanceLayout, scope: &ScopeId) {
    let store = StateStore::new(layout.clone());
    create_requirement(&store, scope, "req_unsourced", RequirementStatus::Active);
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: sid("res_unsourced"),
            title: "Unsourced decision".into(),
            requirement_ids: vec![sid("req_unsourced")],
            supersedes: Vec::new(),
            position: "Adopt".into(),
            rationale: "Settles the requirement".into(),
            status: ResolutionStatus::Proposed,
            context: None,
            enforcement: None,
            confidence: None,
            inputs: Vec::new(),
            made_by: None,
            approved_by: None,
            approved_at: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: sid("rule_unsourced"),
            name: None,
            description: None,
            requirement_ids: vec![sid("req_unsourced")],
            resolution_ids: vec![sid("res_unsourced")],
            statement: "A rule with no source behind it".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::High,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

/// The source leg of the orphan report has to reach the requirement that
/// produces this rule. Another sourced requirement in the scope, however
/// well traced, says nothing about this one.
#[test]
fn orphan_report_wants_a_source_behind_the_producing_requirement() {
    let (_dir, layout, scope) = seeded_layout();
    seed_unsourced_chain(&layout, &scope);

    let orphans = orphan_rules(&layout, &scope).unwrap();
    let orphan_ids: Vec<&str> = orphans
        .iter()
        .map(|orphan| orphan.rule_id.as_str())
        .collect();
    assert_eq!(orphan_ids, vec!["rule_unsourced"]);
    assert_eq!(orphans[0].missing, vec!["source".to_string()]);

    // The gap report names the requirement's missing source, never the
    // rule; the two readers differ only over the source leg.
    let gaps = find_gaps(&layout, &scope).unwrap();
    assert!(!gaps.iter().any(|gap| gap.node_id == "rule_unsourced"));
    assert_eq!(
        coverage_health(&layout, &scope)
            .unwrap()
            .rules
            .with_complete_traceability,
        1
    );
}

/// A rule refines its requirement directly. A resolution may also produce it,
/// but the absence of a resolution is not a traceability gap.
#[test]
fn requirement_produced_rule_does_not_need_a_resolution() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_anchor");
    create_requirement(&store, &scope, "req_half", RequirementStatus::Active);
    attach_source(&store, &scope, "req_half", "source_anchor");
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: sid("rule_half"),
            name: None,
            description: None,
            requirement_ids: vec![sid("req_half")],
            resolution_ids: Vec::new(),
            statement: "A rule that needs no ambiguity resolved".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::High,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();

    assert!(orphan_rules(&layout, &scope).unwrap().is_empty());
    assert!(!find_gaps(&layout, &scope)
        .unwrap()
        .iter()
        .any(|gap| gap.node_id == "rule_half"));
    assert_eq!(
        coverage_health(&layout, &scope)
            .unwrap()
            .rules
            .with_complete_traceability,
        1
    );
}

#[test]
fn rule_without_a_producing_requirement_remains_orphaned() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_anchor");
    create_requirement(&store, &scope, "req_decided", RequirementStatus::Active);
    attach_source(&store, &scope, "req_decided", "source_anchor");
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: sid("res_decided"),
            title: "Decision that does not bind the rule to its requirement".into(),
            requirement_ids: vec![sid("req_decided")],
            supersedes: Vec::new(),
            position: "Adopt".into(),
            rationale: "Settles the requirement".into(),
            status: ResolutionStatus::Proposed,
            context: None,
            enforcement: None,
            confidence: None,
            inputs: Vec::new(),
            made_by: None,
            approved_by: None,
            approved_at: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    append_record(
        &crate::shards::rules_path(&layout, &scope),
        &serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": scope.as_str(), "id": "rule_unattached",
            "statement": "A rule with no producing requirement", "status": "active",
            "severity": "high", "resolution_ids": ["res_decided"]}),
    );

    let orphans = orphan_rules(&layout, &scope).unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].rule_id, "rule_unattached");
    assert_eq!(orphans[0].missing, vec!["source".to_string()]);
    assert!(!find_gaps(&layout, &scope)
        .unwrap()
        .iter()
        .any(|gap| gap.node_id == "rule_unattached"));
}

fn seed_rule_with_producers(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
    has_requirement: bool,
    has_resolution: bool,
) {
    let store = StateStore::new(layout.clone());
    if has_requirement {
        create_source(&store, scope, "source_anchor");
        create_requirement(&store, scope, "req_rule", RequirementStatus::Active);
        attach_source(&store, scope, "req_rule", "source_anchor");
    }
    if has_requirement {
        if has_resolution {
            store
                .create_resolution(CreateResolutionInput {
                    scope_id: scope.clone(),
                    id: sid("res_rule"),
                    title: "Rule decision".into(),
                    requirement_ids: vec![sid("req_rule")],
                    supersedes: Vec::new(),
                    position: "Adopt".into(),
                    rationale: "Decides the rule".into(),
                    status: ResolutionStatus::Proposed,
                    context: None,
                    enforcement: None,
                    confidence: None,
                    inputs: Vec::new(),
                    made_by: None,
                    approved_by: None,
                    approved_at: None,
                    origin_thread: None,
                    origin_message: None,
                })
                .unwrap();
        }
        store
            .create_rule(CreateRuleInput {
                scope_id: scope.clone(),
                id: sid("rule_under_test"),
                name: None,
                description: None,
                requirement_ids: vec![sid("req_rule")],
                resolution_ids: has_resolution
                    .then(|| sid("res_rule"))
                    .into_iter()
                    .collect(),
                statement: "A rule under producer conformance test".into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            })
            .unwrap();
        return;
    }
    // A rule with no requirement is a hand edit the writers refuse.
    if has_resolution {
        append_record(
            &crate::shards::resolutions_path(layout, scope),
            &serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": scope.as_str(), "id": "res_rule",
                "title": "Rule decision", "position": "Adopt", "rationale": "Decides the rule",
                "status": "proposed", "inputs": [], "review_on": null}),
        );
    }
    let resolution_ids: Vec<&str> = has_resolution.then_some("res_rule").into_iter().collect();
    append_record(
        &crate::shards::rules_path(layout, scope),
        &serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": scope.as_str(), "id": "rule_under_test",
            "statement": "A rule under producer conformance test", "status": "active",
            "severity": "high", "resolution_ids": resolution_ids}),
    );
}

/// `orphans` and health read one source test: a rule is complete when a
/// source reaches it through a requirement it names. The gap report never
/// names a rule for its lists; a rule with no requirement is the
/// validator's refusal, not a gap.
#[test]
#[verifies("rule_graph_gaps", conformance)]
fn orphan_health_and_gaps_agree_on_the_source_leg() {
    for has_requirement in [false, true] {
        for has_resolution in [false, true] {
            let (_dir, layout, scope) = empty_layout();
            seed_rule_with_producers(&layout, &scope, has_requirement, has_resolution);

            let orphans = orphan_rules(&layout, &scope).unwrap();
            let orphan = orphans
                .iter()
                .find(|orphan| orphan.rule_id == "rule_under_test");
            assert_eq!(
                orphan.map(|orphan| orphan.missing.clone()),
                (!has_requirement).then(|| vec!["source".to_string()]),
                "orphans disagreed with the source leg for \
                 requirement={has_requirement}, resolution={has_resolution}"
            );
            let health = coverage_health(&layout, &scope).unwrap();
            assert_eq!(
                health.rules.with_complete_traceability,
                usize::from(has_requirement),
                "health disagreed with orphans for requirement={has_requirement}, \
                 resolution={has_resolution}"
            );
            assert!(
                !find_gaps(&layout, &scope)
                    .unwrap()
                    .iter()
                    .any(|gap| gap.node_id == "rule_under_test"),
                "gaps named the rule for requirement={has_requirement}, \
                 resolution={has_resolution}"
            );
        }
    }
}

#[test]
#[provenance_macros::verifies("rule_graph_gaps", examples)]
fn gaps_flag_requirements_without_domain_id_but_not_requirements_with_one() {
    let (_dir, layout, scope) = seeded_layout();
    StateStore::new(layout.clone())
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: sid("req_missing_domain"),
            statement: "Rostering rules need a domain".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let gaps = find_gaps(&layout, &scope).unwrap();
    assert!(gaps.iter().any(|gap| gap.kind == GapKind::MissingDomainId
        && gap.requirement_id.as_deref() == Some("req_missing_domain")
        && gap.reason.contains("domain_id")));
    assert!(!gaps.iter().any(|gap| gap.kind == GapKind::MissingDomainId
        && gap.requirement_id.as_deref() == Some("req_schads_overtime")
        && gap.reason.contains("domain_id")));
}
