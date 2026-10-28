use super::super::{GapItem, GapKind};
use super::fixtures::*;
use provenance_core::{
    NodeType, QuestionStatus, Requirement, RequirementStatus, Resolution, ResolutionStatus, Rule,
    RuleStatus, TopicStatus,
};
use provenance_macros::verifies;

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

#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn each_live_dependent_kind_under_only_archived_parents_is_a_gap() {
    let mut requirements = parents();
    requirements.push(Requirement {
        refines: vec![sid("req_archived")],
        ..requirement("req_child")
    });
    let resolutions = vec![Resolution {
        requirement_ids: vec![sid("req_archived")],
        ..resolution("res_child")
    }];
    let rules = vec![Rule {
        requirement_ids: vec![sid("req_archived")],
        ..rule("rule_child")
    }];
    let topics = vec![topic_for("topic_child", "req_archived", TopicStatus::Open)];
    let questions = vec![question_for(
        "question_child",
        "topic_child",
        "req_archived",
        QuestionStatus::Open,
    )];
    let gaps = compute_for(
        &[],
        &requirements,
        &resolutions,
        &rules,
        &topics,
        &questions,
        &[],
    );
    assert_eq!(
        no_live_parent(&gaps),
        [
            (NodeType::Requirement, "req_child", Some("req_archived")),
            (NodeType::Resolution, "res_child", Some("req_archived")),
            (NodeType::Rule, "rule_child", Some("req_archived")),
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

/// A live parent, a missing parent, an archived child, an ended decision and a
/// root Requirement are not this gap.
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
    let resolutions = [ResolutionStatus::Abandoned, ResolutionStatus::Rejected]
        .into_iter()
        .map(|status| Resolution {
            status: status.clone(),
            requirement_ids: vec![sid("req_archived")],
            ..resolution(&format!("res_{status:?}").to_lowercase())
        })
        .collect::<Vec<_>>();
    let rules = vec![Rule {
        status: RuleStatus::Archived,
        requirement_ids: vec![sid("req_archived")],
        ..rule("rule_archived")
    }];
    let gaps = compute_for(&[], &requirements, &resolutions, &rules, &[], &[], &[]);
    let found = no_live_parent(&gaps);
    assert!(found.is_empty(), "{found:?}");
}
