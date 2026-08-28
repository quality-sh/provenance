//! What `trace_rule` walks back from a rule.
//!
//! A rule can be produced two ways at once: named directly by a requirement,
//! and named by a resolution that settles some other requirement. Both
//! producers sit at the `from` end of a produces edge, and both requirements
//! belong in the answer.

use super::super::*;
use super::fixtures::*;
use crate::state_store::{CreateResolutionInput, CreateRuleInput, StateStore};
use provenance_core::{
    EdgeType, RequirementStatus, ResolutionStatus, RuleSeverity, RuleStatus, ScopeId,
};

/// A rule with both kinds of producer: `req_direct` names it outright, and
/// `res_indirect` produces it while resolving `req_resolved`. Each
/// requirement cites its own source.
fn seed_two_producers(layout: &ProvenanceLayout, scope: &ScopeId) {
    let store = StateStore::new(layout.clone());
    create_requirement(&store, scope, "req_direct", RequirementStatus::Active);
    create_requirement(&store, scope, "req_resolved", RequirementStatus::Active);
    create_source(&store, scope, "source_direct");
    create_source(&store, scope, "source_resolved");
    attach_source(&store, scope, "req_direct", "source_direct");
    attach_source(&store, scope, "req_resolved", "source_resolved");
    store
        .create_resolution(
            None,
            CreateResolutionInput {
                scope_id: scope.clone(),
                id: sid("res_indirect"),
                title: "Indirect decision".into(),
                requirement_id: Some(sid("req_resolved")),
                position: "Adopt".into(),
                rationale: "Settles the other requirement".into(),
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
                id: sid("rule_two_producers"),
                name: None,
                description: None,
                requirement_id: Some(sid("req_direct")),
                resolution_id: Some(sid("res_indirect")),
                statement: "Both producers are recorded".into(),
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

/// Sorted, so a test compares sets rather than whatever order the walk
/// happened to build.
fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

#[test]
fn a_rule_named_by_a_requirement_and_a_resolution_traces_back_to_both_requirements() {
    let (_dir, layout, scope) = empty_layout();
    seed_two_producers(&layout, &scope);

    let trace = trace_rule(&layout, &scope, &sid("rule_two_producers")).unwrap();

    assert_eq!(
        sorted(
            trace
                .requirements
                .iter()
                .map(|requirement| requirement.id.as_str().to_string())
                .collect()
        ),
        vec!["req_direct".to_string(), "req_resolved".to_string()]
    );
    assert_eq!(
        trace
            .resolutions
            .iter()
            .map(|resolution| resolution.id.as_str().to_string())
            .collect::<Vec<_>>(),
        vec!["res_indirect".to_string()]
    );
}

/// The direct producer's source is the one the old walk lost: it hung off a
/// requirement the walk never reached.
#[test]
fn both_requirements_bring_their_own_sources() {
    let (_dir, layout, scope) = empty_layout();
    seed_two_producers(&layout, &scope);

    let trace = trace_rule(&layout, &scope, &sid("rule_two_producers")).unwrap();

    assert_eq!(
        sorted(
            trace
                .sources
                .iter()
                .map(|source| source.id.as_str().to_string())
                .collect()
        ),
        vec!["source_direct".to_string(), "source_resolved".to_string()]
    );
}

/// Exactly the edges the walk crossed: two produces, one resolves, two
/// references. The `needs` edge from `req_resolved` to `res_indirect` is in
/// the scope but off the walk, and the rule's own id is never mistaken for a
/// requirement.
#[test]
fn only_the_walked_edges_are_returned() {
    let (_dir, layout, scope) = empty_layout();
    seed_two_producers(&layout, &scope);

    let trace = trace_rule(&layout, &scope, &sid("rule_two_producers")).unwrap();

    let mut walked = trace
        .edges
        .iter()
        .map(|edge| {
            format!(
                "{:?} {} -> {}",
                edge.edge_type,
                edge.from_id.as_str(),
                edge.to_id.as_str()
            )
        })
        .collect::<Vec<_>>();
    walked.sort();
    assert_eq!(
        walked,
        sorted(vec![
            format!("{:?} req_direct -> rule_two_producers", EdgeType::Produces),
            format!(
                "{:?} res_indirect -> rule_two_producers",
                EdgeType::Produces
            ),
            format!("{:?} res_indirect -> req_resolved", EdgeType::Resolves),
            format!("{:?} source_direct -> req_direct", EdgeType::References),
            format!("{:?} source_resolved -> req_resolved", EdgeType::References),
        ])
    );
    assert!(!trace
        .requirements
        .iter()
        .any(|requirement| requirement.id.as_str() == "rule_two_producers"));
    assert_eq!(trace.rule.id.as_str(), "rule_two_producers");
}
