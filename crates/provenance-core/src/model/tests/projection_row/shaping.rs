use super::assert_kind_round_trips;
use crate::model::{
    ArchivedStamp, ArtifactLink, ArtifactLinkTargetType, Question, QuestionStatus,
    ResolutionMethod, ScopeId, StableId, Topic, TopicStatus,
};

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn links() -> Vec<ArtifactLink> {
    vec![
        ArtifactLink {
            target_type: ArtifactLinkTargetType::Requirement,
            target_id: sid("req_overtime"),
        },
        ArtifactLink {
            target_type: ArtifactLinkTargetType::Rule,
            target_id: sid("rule_overtime_001"),
        },
    ]
}

fn filled_topic() -> Topic {
    Topic {
        scope_id: scope(),
        id: sid("topic_rates"),
        requirement_id: sid("req_overtime"),
        title: "Rates".into(),
        status: TopicStatus::Archived,
        archived_in_commit: Some(ArchivedStamp {
            commit: "a".repeat(40),
            at: None,
        }),
        claimed_by: Some("ben".into()),
        claimed_at: Some(1_700_000_000),
        links: links(),
    }
}

fn bare_topic() -> Topic {
    Topic {
        scope_id: scope(),
        id: sid("topic_bare"),
        requirement_id: sid("req_overtime"),
        title: "Bare".into(),
        status: TopicStatus::Open,
        archived_in_commit: None,
        claimed_by: None,
        claimed_at: None,
        links: Vec::new(),
    }
}

fn filled_question() -> Question {
    Question {
        depends_on: Vec::new(),
        scope_id: scope(),
        id: sid("question_threshold"),
        topic_id: sid("topic_rates"),
        requirement_id: sid("req_overtime"),
        question: "Which threshold applies?".into(),
        resolution_method: ResolutionMethod::Research,
        status: QuestionStatus::Archived,
        archived_in_commit: Some(ArchivedStamp {
            commit: "a".repeat(40),
            at: None,
        }),
        claimed_by: Some("ben".into()),
        claimed_at: Some(1_700_000_000),
        answer: Some("The award threshold".into()),
        links: links(),
        resolution_id: Some(sid("res_overtime")),
        contradicts: Some(sid("req_penalty")),
    }
}

fn bare_question() -> Question {
    Question {
        depends_on: Vec::new(),
        scope_id: scope(),
        id: sid("question_bare"),
        topic_id: sid("topic_rates"),
        requirement_id: sid("req_overtime"),
        question: "Bare?".into(),
        resolution_method: ResolutionMethod::Grill,
        status: QuestionStatus::Open,
        archived_in_commit: None,
        claimed_by: None,
        claimed_at: None,
        answer: None,
        links: Vec::new(),
        resolution_id: None,
        contradicts: None,
    }
}

#[test]
fn a_topic_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_topic(), &bare_topic());
}

#[test]
fn a_question_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_question(), &bare_question());
}
