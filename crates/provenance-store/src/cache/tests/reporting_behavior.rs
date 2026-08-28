use super::super::*;
use super::fixtures::*;
use crate::state_store::{
    CreateRequirementInput, CreateResolutionInput, CreateRuleInput, StateStore,
};
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
        .create_resolution(
            None,
            CreateResolutionInput {
                scope_id: scope.clone(),
                id: sid("res_unsourced"),
                title: "Unsourced decision".into(),
                requirement_id: Some(sid("req_unsourced")),
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
                superseded_by: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
    store
        .create_rule(
            None,
            CreateRuleInput {
                scope_id: scope.clone(),
                id: sid("rule_unsourced"),
                name: None,
                description: None,
                requirement_id: Some(sid("req_unsourced")),
                resolution_id: Some(sid("res_unsourced")),
                statement: "A rule with no source behind it".into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            },
        )
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

    // Both producers are recorded, so the gap report leaves the rule alone;
    // the two readers differ only over the source leg.
    let gaps = find_gaps(&layout, &scope).unwrap();
    assert!(!gaps
        .iter()
        .any(|gap| gap.kind == GapKind::OrphanRule && gap.node_id == "rule_unsourced"));
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
        .create_rule(
            None,
            CreateRuleInput {
                scope_id: scope.clone(),
                id: sid("rule_half"),
                name: None,
                description: None,
                requirement_id: Some(sid("req_half")),
                resolution_id: None,
                statement: "A rule that needs no ambiguity resolved".into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();

    assert!(orphan_rules(&layout, &scope).unwrap().is_empty());
    assert!(!find_gaps(&layout, &scope)
        .unwrap()
        .iter()
        .any(|gap| gap.kind == GapKind::OrphanRule && gap.node_id == "rule_half"));
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
        .create_resolution(
            None,
            CreateResolutionInput {
                scope_id: scope.clone(),
                id: sid("res_decided"),
                title: "Decision that does not bind the rule to its requirement".into(),
                requirement_id: Some(sid("req_decided")),
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
                superseded_by: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
    store
        .create_rule(
            None,
            CreateRuleInput {
                scope_id: scope.clone(),
                id: sid("rule_unattached"),
                name: None,
                description: None,
                requirement_id: None,
                resolution_id: Some(sid("res_decided")),
                statement: "A rule with no producing requirement".into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();

    let orphans = orphan_rules(&layout, &scope).unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].rule_id, "rule_unattached");
    assert_eq!(
        orphans[0].missing,
        vec!["requirement".to_string(), "source".to_string()]
    );
    assert!(find_gaps(&layout, &scope).unwrap().iter().any(|gap| {
        gap.kind == GapKind::OrphanRule
            && gap.node_id == "rule_unattached"
            && gap.reason == "no requirement produces this rule"
    }));
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
    if has_resolution {
        store
            .create_resolution(
                None,
                CreateResolutionInput {
                    scope_id: scope.clone(),
                    id: sid("res_rule"),
                    title: "Rule decision".into(),
                    requirement_id: has_requirement.then(|| sid("req_rule")),
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
                    superseded_by: None,
                    origin_thread: None,
                    origin_message: None,
                },
            )
            .unwrap();
    }
    store
        .create_rule(
            None,
            CreateRuleInput {
                scope_id: scope.clone(),
                id: sid("rule_under_test"),
                name: None,
                description: None,
                requirement_id: has_requirement.then(|| sid("req_rule")),
                resolution_id: has_resolution.then(|| sid("res_rule")),
                statement: "A rule under producer conformance test".into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
}

/// `orphans` and `gaps` are two consumers of one producer test, so they must
/// name the same rules for every shape of producer edge. The expectation is
/// derived from `RuleProducer::REQUIRED` rather than restated here: which
/// producers a rule owes is the graph's ruling to change, and this test holds
/// the two consumers together through any such change.
#[test]
#[verifies("rule_graph_gaps", conformance)]
fn orphan_health_and_gaps_name_the_same_rules() {
    use crate::cache::gaps::graph_query::RuleProducer;

    for has_requirement in [false, true] {
        for has_resolution in [false, true] {
            let (_dir, layout, scope) = empty_layout();
            seed_rule_with_producers(&layout, &scope, has_requirement, has_resolution);

            let expected_missing = RuleProducer::REQUIRED
                .into_iter()
                .filter(|producer| match producer {
                    RuleProducer::Requirement => !has_requirement,
                    RuleProducer::Resolution => !has_resolution,
                })
                .map(|producer: RuleProducer| producer.word().to_string())
                .collect::<Vec<_>>();

            let health_missing = orphan_rules(&layout, &scope)
                .unwrap()
                .into_iter()
                .find(|orphan| orphan.rule_id == "rule_under_test")
                .map(|orphan| {
                    orphan
                        .missing
                        .into_iter()
                        .filter(|missing| missing != "source")
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let gap = find_gaps(&layout, &scope)
                .unwrap()
                .into_iter()
                .find(|gap| gap.kind == GapKind::OrphanRule && gap.node_id == "rule_under_test");

            assert_eq!(
                health_missing, expected_missing,
                "health disagreed with the required-producer list for \
                 requirement={has_requirement}, resolution={has_resolution}"
            );
            assert_eq!(
                gap.is_some(),
                !expected_missing.is_empty(),
                "gaps disagreed with health for requirement={has_requirement}, \
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
        .create_requirement(
            None,
            CreateRequirementInput {
                scope_id: scope.clone(),
                id: sid("req_missing_domain"),
                statement: "Rostering rules need a domain".into(),
                description: None,
                status: RequirementStatus::Active,
                domain_id: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
    let gaps = find_gaps(&layout, &scope).unwrap();
    assert!(gaps.iter().any(|gap| gap.kind == GapKind::MissingDomainId
        && gap.requirement_id.as_deref() == Some("req_missing_domain")
        && gap.reason.contains("domain_id")));
    assert!(!gaps.iter().any(|gap| gap.kind == GapKind::MissingDomainId
        && gap.requirement_id.as_deref() == Some("req_schads_overtime")
        && gap.reason.contains("domain_id")));
}
