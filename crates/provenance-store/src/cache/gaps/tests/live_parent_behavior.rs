use super::super::{compute_gaps, GapGraph, GapItem, GapKind};
use super::fixtures::*;
use provenance_core::{
    Boundary, NodeType, QuestionStatus, Requirement, RequirementStatus, Resolution,
    ResolutionStatus, Rule, RuleStatus, ScopeId, TopicStatus, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::verifies;

// `rule_live_dependents_have_live_parents` settles the obligation only for
// Requirements and Rules. For Resolutions, Topics and Questions it waits on the
// open question `q_record_lifecycle_parent_scope`, so the tests of those kinds
// have no Rule binding.

fn parents() -> Vec<Requirement> {
    vec![
        Requirement {
            status: RequirementStatus::Archived,
            ..requirement("req_archived")
        },
        requirement("req_live"),
    ]
}

fn no_live_parent(gaps: &[GapItem]) -> Vec<(NodeType, &str, Option<&str>)> {
    gaps.iter()
        .filter(|gap| gap.kind == GapKind::NoLiveParent)
        .map(|gap| {
            (
                gap.node_type,
                gap.node_id.as_str(),
                gap.related_node_id.as_deref(),
            )
        })
        .collect()
}

fn boundary(id: &str, requirement_id: &str) -> Boundary {
    serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": id,
        "requirement_id": requirement_id, "statement": "Back pay is out of scope"
    }))
    .unwrap()
}

#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn a_live_requirement_or_rule_under_only_archived_parents_is_a_gap() {
    let mut requirements = parents();
    requirements.push(Requirement {
        refines: vec![sid("req_archived")],
        ..requirement("req_child")
    });
    let rules = vec![Rule {
        requirement_ids: vec![sid("req_archived")],
        ..rule("rule_child")
    }];
    let gaps = compute_for(&[], &requirements, &[], &rules, &[], &[], &[]);
    assert_eq!(
        no_live_parent(&gaps),
        [
            (NodeType::Requirement, "req_child", Some("req_archived")),
            (NodeType::Rule, "rule_child", Some("req_archived")),
        ]
    );
}

/// A live parent, a missing parent, an archived child and a root Requirement
/// are not this gap.
#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn a_record_with_a_live_parent_or_no_lifecycle_is_not_a_gap() {
    let mut requirements = parents();
    requirements.extend([
        Requirement {
            refines: vec![sid("req_archived"), sid("req_live")],
            ..requirement("req_survives")
        },
        Requirement {
            refines: vec![sid("req_missing")],
            ..requirement("req_dangling")
        },
        Requirement {
            status: RequirementStatus::Archived,
            refines: vec![sid("req_archived")],
            ..requirement("req_also_archived")
        },
    ]);
    let rules = vec![
        Rule {
            requirement_ids: vec![sid("req_archived"), sid("req_live")],
            ..rule("rule_survives")
        },
        Rule {
            status: RuleStatus::Archived,
            requirement_ids: vec![sid("req_archived")],
            ..rule("rule_archived")
        },
    ];
    let gaps = compute_for(&[], &requirements, &[], &rules, &[], &[], &[]);
    let found = no_live_parent(&gaps);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_live_resolution_topic_or_question_under_only_archived_parents_is_a_gap() {
    let resolutions = vec![Resolution {
        requirement_ids: vec![sid("req_archived")],
        ..resolution("res_child")
    }];
    let topics = vec![topic_for("topic_child", "req_archived", TopicStatus::Open)];
    let questions = vec![question_for(
        "question_child",
        "topic_child",
        "req_archived",
        QuestionStatus::Open,
    )];
    let gaps = compute_for(&[], &parents(), &resolutions, &[], &topics, &questions, &[]);
    assert_eq!(
        no_live_parent(&gaps),
        [
            (NodeType::Resolution, "res_child", Some("req_archived")),
            (NodeType::Topic, "topic_child", Some("req_archived")),
            (NodeType::Question, "question_child", Some("req_archived")),
        ]
    );
    let question = gaps
        .iter()
        .find(|gap| gap.kind == GapKind::NoLiveParent && gap.node_id == "question_child")
        .unwrap();
    assert_eq!(question.requirement_id.as_deref(), Some("req_archived"));
}

/// An ended or archived decision, an archived Topic or Question, a Topic or
/// Question with a live parent, and a Boundary are not this gap. A Boundary
/// has no lifecycle status.
#[test]
fn an_ended_or_archived_record_or_a_boundary_is_not_a_gap() {
    let resolutions = [
        ResolutionStatus::Abandoned,
        ResolutionStatus::Rejected,
        ResolutionStatus::Archived,
    ]
    .into_iter()
    .map(|status| Resolution {
        status: status.clone(),
        requirement_ids: vec![sid("req_archived")],
        ..resolution(&format!("res_{status:?}").to_lowercase())
    })
    .collect::<Vec<_>>();
    let topics = vec![
        topic_for("topic_archived", "req_archived", TopicStatus::Archived),
        topic_for("topic_live", "req_live", TopicStatus::Open),
    ];
    let questions = vec![
        question_for(
            "question_archived",
            "topic_archived",
            "req_archived",
            QuestionStatus::Archived,
        ),
        question_for(
            "question_live",
            "topic_live",
            "req_live",
            QuestionStatus::Open,
        ),
    ];
    let scope = ScopeId::new("default").unwrap();
    let gaps = compute_gaps(&GapGraph {
        scope: &scope,
        sources: &[],
        requirements: &parents(),
        resolutions: &resolutions,
        rules: &[],
        topics: &topics,
        questions: &questions,
        threads: &[],
        domains: &[],
        boundaries: &[boundary("boundary_archived_parent", "req_archived")],
        contributions: &[],
        synthesis_packets: &[],
    });
    let found = no_live_parent(&gaps);
    assert!(found.is_empty(), "{found:?}");
}
