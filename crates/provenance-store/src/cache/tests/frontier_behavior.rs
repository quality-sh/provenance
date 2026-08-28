use super::super::*;
use super::fixtures::*;
use super::gap_rule_behavior::all_gap_kinds;
use crate::state_store::{
    AddSourceReferenceInput, CreateEdgeInput, CreateProposalCardInput, CreateQuestionInput,
    CreateResolutionInput, CreateRuleInput, CreateSourceInput, CreateTopicInput, StateStore,
};
use provenance_core::{
    EdgeType, IdeationTarget, IdeationTargetType, NodeType, PromotionState, ProposalTraceability,
    ProposalType, QuestionStatus, RequirementStatus, ResolutionMethod, ResolutionStatus,
    RuleSeverity, RuleStatus, SourceType, TopicStatus,
};
use provenance_macros::verifies;

fn add_topic_question(
    store: &StateStore,
    scope: &provenance_core::ScopeId,
    requirement: &str,
    topic: &str,
    question: &str,
    status: QuestionStatus,
) {
    store
        .create_topic(
            None,
            CreateTopicInput {
                scope_id: scope.clone(),
                id: sid(topic),
                requirement_id: sid(requirement),
                title: topic.to_string(),
                status: TopicStatus::Open,
                links: Vec::new(),
            },
        )
        .unwrap();
    store
        .create_question(
            None,
            CreateQuestionInput {
                scope_id: scope.clone(),
                id: sid(question),
                topic_id: sid(topic),
                question: "Which path should this take?".into(),
                resolution_method: ResolutionMethod::Grill,
                status,
                answer: None,
                links: Vec::new(),
                resolution_id: None,
            },
        )
        .unwrap();
}

fn assert_gap(gaps: &[GapItem], kind: GapKind, node_type: NodeType, node_id: &str) {
    assert!(
        gaps.iter()
            .any(|gap| gap.kind == kind && gap.node_type == node_type && gap.node_id == node_id),
        "missing {kind:?} for {node_type:?} {node_id}; got {gaps:#?}"
    );
}

#[test]
fn ordinary_prime_does_not_expand_into_a_global_proposal_queue() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    store
        .create_proposal_card(
            None,
            CreateProposalCardInput {
                scope_id: scope.clone(),
                id: sid("proposal_demand_only"),
                proposal_key: "demand-only".into(),
                proposal_type: ProposalType::RequirementCandidate,
                title: "Demand only".into(),
                summary: "Surface only when territory is claimed".into(),
                confidence: None,
                traceability: ProposalTraceability {
                    target: IdeationTarget {
                        artifact_type: IdeationTargetType::Requirement,
                        artifact_id: sid("req_demand_only"),
                    },
                    source_ids: Vec::new(),
                    evidence_references: Vec::new(),
                    supporting_claim_ids: Vec::new(),
                },
                builds_on: Vec::new(),
                promotion_state: PromotionState::Proposed,
                duplicate_of: None,
                superseded_by: None,
            },
        )
        .unwrap();

    let prime = prime_context(&layout, &scope, false).unwrap();

    assert!(!serde_json::to_string(&prime)
        .unwrap()
        .contains("proposal_demand_only"));
    assert!(!render_prime_markdown(&prime).contains("proposal_demand_only"));
}

#[test]
#[allow(clippy::too_many_lines)]
#[verifies("rule_graph_gaps", examples)]
fn find_gaps_reports_the_frontier_taxonomy() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_anchor");
    create_source(&store, &scope, "source_unused");
    store
        .create_source(
            None,
            CreateSourceInput {
                scope_id: scope.clone(),
                id: sid("source_dangling"),
                name: "source_dangling".into(),
                source_type: SourceType::Policy,
                url: None,
                reference: None,
                commit_pin: None,
                effective_date: None,
                review_date: None,
                superseded_by: Some(sid("source_missing")),
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
    for (id, status) in [
        ("req_missing_source", RequirementStatus::Active),
        ("req_resolved_no_decision", RequirementStatus::Resolved),
        ("req_decided_no_rule", RequirementStatus::Active),
        ("req_contradicts_a", RequirementStatus::Active),
        ("req_contradicts_b", RequirementStatus::Active),
        ("req_question_topic", RequirementStatus::Active),
    ] {
        create_requirement(&store, &scope, id, status);
        if id != "req_missing_source" {
            attach_source(&store, &scope, id, "source_anchor");
        }
    }
    store
        .add_source_reference(
            None,
            AddSourceReferenceInput {
                scope_id: scope.clone(),
                source_id: sid("source_dangling"),
                requirement_id: sid("req_question_topic"),
                clause: None,
            },
        )
        .unwrap();
    store
        .create_resolution(
            None,
            CreateResolutionInput {
                scope_id: scope.clone(),
                id: sid("res_decision_without_rule"),
                title: "res_decision_without_rule".into(),
                requirement_id: Some(sid("req_decided_no_rule")),
                position: "Adopt".into(),
                rationale: "Resolves frontier".into(),
                status: ResolutionStatus::Approved,
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
        .create_resolution(
            None,
            CreateResolutionInput {
                scope_id: scope.clone(),
                id: sid("res_orphan"),
                title: "res_orphan".into(),
                requirement_id: None,
                position: "Adopt".into(),
                rationale: "Resolves frontier".into(),
                status: ResolutionStatus::Approved,
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
                id: sid("rule_orphan"),
                name: None,
                description: None,
                requirement_id: None,
                resolution_id: None,
                statement: "An unattached rule exists".into(),
                status: RuleStatus::Active,
                severity: RuleSeverity::High,
                source_document: None,
                source_section: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
    store
        .create_edge(
            None,
            CreateEdgeInput {
                scope_id: scope.clone(),
                edge_type: EdgeType::Contradicts,
                from_type: NodeType::Requirement,
                from_id: sid("req_contradicts_a"),
                to_type: NodeType::Requirement,
                to_id: sid("req_contradicts_b"),
            },
        )
        .unwrap();
    add_topic_question(
        &store,
        &scope,
        "req_question_topic",
        "topic_frontier",
        "question_frontier",
        QuestionStatus::Open,
    );

    let gaps = find_gaps(&layout, &scope).unwrap();
    for (kind, node_type, id) in [
        (
            GapKind::MissingSourceRefs,
            NodeType::Requirement,
            "req_missing_source",
        ),
        (
            GapKind::NoResolvingDecision,
            NodeType::Requirement,
            "req_resolved_no_decision",
        ),
        (
            GapKind::NoProducedRules,
            NodeType::Requirement,
            "req_decided_no_rule",
        ),
        (
            GapKind::OrphanResolution,
            NodeType::Resolution,
            "res_orphan",
        ),
        (GapKind::OrphanRule, NodeType::Rule, "rule_orphan"),
        (
            GapKind::UnreferencedSource,
            NodeType::Source,
            "source_unused",
        ),
        (
            GapKind::DanglingReference,
            NodeType::Source,
            "source_dangling",
        ),
        (
            GapKind::UnresolvedContradictsPair,
            NodeType::Requirement,
            "req_contradicts_a",
        ),
        (
            GapKind::OpenQuestion,
            NodeType::Question,
            "question_frontier",
        ),
        (GapKind::UnexploredTopic, NodeType::Topic, "topic_frontier"),
    ] {
        assert_gap(&gaps, kind, node_type, id);
    }
    // Every kind has to survive the trip through the state store, not just
    // through `compute_gaps` on hand-built records.
    for kind in all_gap_kinds() {
        assert!(
            gaps.iter().any(|gap| gap.kind == kind),
            "the taxonomy graph reports no {kind:?} gap; got {gaps:#?}"
        );
    }
    let contradiction = gaps
        .iter()
        .find(|gap| gap.kind == GapKind::UnresolvedContradictsPair)
        .unwrap();
    assert_eq!(contradiction.related_node_type, Some(NodeType::Requirement));
    assert_eq!(
        contradiction.related_node_id.as_deref(),
        Some("req_contradicts_b")
    );
    assert_eq!(prime_context(&layout, &scope, false).unwrap().gaps, gaps);
}

#[test]
fn prime_renders_frontier_gap_subjects() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_anchor");
    for id in [
        "req_contradicts_a",
        "req_contradicts_b",
        "req_question_topic",
    ] {
        create_requirement(&store, &scope, id, RequirementStatus::Active);
        attach_source(&store, &scope, id, "source_anchor");
    }
    store
        .create_edge(
            None,
            CreateEdgeInput {
                scope_id: scope.clone(),
                edge_type: EdgeType::Contradicts,
                from_type: NodeType::Requirement,
                from_id: sid("req_contradicts_a"),
                to_type: NodeType::Requirement,
                to_id: sid("req_contradicts_b"),
            },
        )
        .unwrap();
    add_topic_question(
        &store,
        &scope,
        "req_question_topic",
        "topic_frontier",
        "question_frontier",
        QuestionStatus::Open,
    );
    let rendered = render_prime_markdown(&prime_context(&layout, &scope, false).unwrap());
    assert!(rendered.contains("- requirement req_contradicts_a -> requirement req_contradicts_b: unresolved `contradicts` pair"));
    assert!(rendered.contains("- question question_frontier: open question"));
    assert!(rendered.contains("- topic topic_frontier: unexplored topic"));
}

#[test]
#[verifies("rule_graph_gaps", examples)]
fn prime_renders_blocked_on_human_questions_with_status() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_anchor");
    create_requirement(
        &store,
        &scope,
        "req_blocked_question",
        RequirementStatus::Active,
    );
    attach_source(&store, &scope, "req_blocked_question", "source_anchor");
    add_topic_question(
        &store,
        &scope,
        "req_blocked_question",
        "topic_blocked",
        "question_blocked",
        QuestionStatus::BlockedOnHuman,
    );
    let gaps = find_gaps(&layout, &scope).unwrap();
    let blocked = gaps
        .iter()
        .find(|gap| gap.kind == GapKind::OpenQuestion && gap.node_id == "question_blocked")
        .unwrap();
    assert!(blocked.reason.contains("blocked_on_human"));
    let rendered = render_prime_markdown(&prime_context(&layout, &scope, false).unwrap());
    assert!(rendered.contains("- question question_blocked:"));
    assert!(rendered.contains("blocked_on_human"));
}
