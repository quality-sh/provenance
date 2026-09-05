use super::super::GapKind;
use super::fixtures::*;
use provenance_core::{
    NodeType, Question, QuestionStatus, Requirement, RequirementStatus, Resolution,
    ResolutionStatus, Rule, SourceReference, TopicStatus,
};
use provenance_macros::verifies;

/// A question on `req_left` naming `req_right` as contradicted, with the
/// explored topic it sits under.
fn contradiction(question_id: &str, requirement: &str, other: &str) -> Question {
    Question {
        contradicts: Some(sid(other)),
        ..question_for(question_id, "topic_pair", requirement, QuestionStatus::Open)
    }
}

fn pair_topic() -> Vec<provenance_core::Topic> {
    vec![topic_for("topic_pair", "req_left", TopicStatus::Explored)]
}

#[test]
#[verifies("rule_graph_gaps", examples)]
fn a_question_with_a_resolution_settles_its_contradiction() {
    let requirements = vec![requirement("req_left"), requirement("req_right")];
    let resolutions = vec![Resolution {
        requirement_ids: vec![sid("req_left")],
        ..resolution("res_settled")
    }];
    let questions = vec![Question {
        resolution_id: Some(sid("res_settled")),
        ..contradiction("question_pair", "req_left", "req_right")
    }];
    let gaps = compute_for(
        &[],
        &requirements,
        &resolutions,
        &[],
        &pair_topic(),
        &questions,
        &[],
    );
    assert_eq!(count_kind(&gaps, GapKind::UnresolvedContradictsPair), 0);
}

#[test]
#[verifies("rule_graph_gaps", examples)]
fn approved_resolution_need_not_produce_a_rule_but_resolved_requirement_still_does() {
    let requirements = vec![Requirement {
        status: RequirementStatus::Resolved,
        ..requirement("req_resolved")
    }];
    let resolutions = vec![Resolution {
        status: ResolutionStatus::Approved,
        requirement_ids: vec![sid("req_resolved")],
        ..resolution("res_approved")
    }];

    let gaps = compute_for(&[], &requirements, &resolutions, &[], &[], &[], &[]);
    assert!(gaps.iter().any(|gap| {
        gap.kind == GapKind::NoProducedRules
            && gap.node_type == NodeType::Requirement
            && gap.node_id == "req_resolved"
    }));
    assert!(!gaps.iter().any(|gap| {
        gap.kind == GapKind::NoProducedRules
            && gap.node_type == NodeType::Resolution
            && gap.node_id == "res_approved"
    }));
}

#[test]
#[verifies("rule_graph_gaps", examples)]
fn supersession_settles_a_contradiction() {
    let requirements = vec![
        requirement("req_left"),
        Requirement {
            supersedes: vec![sid("req_left")],
            ..requirement("req_right")
        },
    ];
    let questions = vec![contradiction("question_pair", "req_left", "req_right")];
    let gaps = compute_for(&[], &requirements, &[], &[], &pair_topic(), &questions, &[]);
    assert_eq!(count_kind(&gaps, GapKind::UnresolvedContradictsPair), 0);
}

#[test]
#[verifies("rule_graph_gaps", examples)]
fn answered_questions_and_explored_topics_are_not_frontier_gaps() {
    let requirements = vec![requirement("req_topic")];
    let topics = vec![topic("topic_explored", TopicStatus::Explored)];
    let questions = vec![question(
        "question_answered",
        "topic_explored",
        QuestionStatus::Answered,
    )];
    let gaps = compute_for(&[], &requirements, &[], &[], &topics, &questions, &[]);
    assert_eq!(count_kind(&gaps, GapKind::OpenQuestion), 0);
    assert_eq!(count_kind(&gaps, GapKind::UnexploredTopic), 0);
}

#[test]
#[verifies("rule_graph_gaps", examples)]
fn a_rule_naming_a_missing_resolution_has_a_dangling_reference_gap() {
    let rules = vec![Rule {
        requirement_ids: vec![sid("req_anchor")],
        resolution_ids: vec![sid("res_missing")],
        ..rule("rule_dangling")
    }];
    let gaps = compute_for(
        &[],
        &[requirement("req_anchor")],
        &[],
        &rules,
        &[],
        &[],
        &[],
    );
    assert!(gaps.iter().any(|gap| {
        gap.kind == GapKind::DanglingReference
            && gap.node_type == NodeType::Rule
            && gap.node_id == "rule_dangling"
            && gap.related_node_type == Some(NodeType::Resolution)
            && gap.related_node_id.as_deref() == Some("res_missing")
            && gap.reason == "resolution_ids points at missing resolution res_missing"
    }));
}

#[test]
#[verifies("rule_graph_gaps", examples)]
fn a_contradiction_named_from_both_sides_is_reported_once() {
    let sources = vec![source("source_anchor")];
    let mut requirements = vec![requirement("req_left"), requirement("req_right")];
    for requirement in &mut requirements {
        requirement.source_refs = vec![SourceReference {
            source_id: sid("source_anchor"),
            clause: None,
        }];
    }
    let questions = vec![
        contradiction("question_left", "req_left", "req_right"),
        Question {
            topic_id: sid("topic_pair"),
            ..contradiction("question_right", "req_right", "req_left")
        },
    ];
    let gaps = compute_for(
        &sources,
        &requirements,
        &[],
        &[],
        &pair_topic(),
        &questions,
        &[],
    );
    assert_eq!(count_kind(&gaps, GapKind::UnresolvedContradictsPair), 1);
    assert_eq!(count_kind(&gaps, GapKind::OpenQuestion), 0);
}
