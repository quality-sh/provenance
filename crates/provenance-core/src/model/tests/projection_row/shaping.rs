use super::assert_kind_round_trips;
use crate::model::{
    ArtifactLink, ArtifactLinkTargetType, Boundary, Domain, Question, QuestionStatus,
    ResolutionMethod, ScopeId, SourceReference, StableId, Topic, TopicStatus,
};
use crate::SUPPORTED_SCHEMA_VERSION;

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

fn filled_boundary() -> Boundary {
    Boundary {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("boundary_no_backpay"),
        requirement_id: sid("req_overtime"),
        statement: "Back pay is out of scope".into(),
        source_ref: Some(SourceReference {
            source_id: sid("source_schads"),
            clause: Some("cl_3".into()),
        }),
    }
}

fn bare_boundary() -> Boundary {
    Boundary {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("boundary_bare"),
        requirement_id: sid("req_overtime"),
        statement: "Bare".into(),
        source_ref: None,
    }
}

fn filled_topic() -> Topic {
    Topic {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("topic_rates"),
        requirement_id: sid("req_overtime"),
        title: "Rates".into(),
        status: TopicStatus::Explored,
        claimed_by: Some("ben".into()),
        claimed_at: Some(1_700_000_000),
        links: links(),
    }
}

fn bare_topic() -> Topic {
    Topic {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("topic_bare"),
        requirement_id: sid("req_overtime"),
        title: "Bare".into(),
        status: TopicStatus::Open,
        claimed_by: None,
        claimed_at: None,
        links: Vec::new(),
    }
}

fn filled_question() -> Question {
    Question {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("question_threshold"),
        topic_id: sid("topic_rates"),
        requirement_id: sid("req_overtime"),
        question: "Which threshold applies?".into(),
        resolution_method: ResolutionMethod::Research,
        status: QuestionStatus::Answered,
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
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("question_bare"),
        topic_id: sid("topic_rates"),
        requirement_id: sid("req_overtime"),
        question: "Bare?".into(),
        resolution_method: ResolutionMethod::Grill,
        status: QuestionStatus::Open,
        claimed_by: None,
        claimed_at: None,
        answer: None,
        links: Vec::new(),
        resolution_id: None,
        contradicts: None,
    }
}

fn filled_domain() -> Domain {
    Domain {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("domain_payroll"),
        name: "Payroll".into(),
        description: Some("Wages and overtime".into()),
        color: Some("#00ff00".into()),
    }
}

fn bare_domain() -> Domain {
    Domain {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("domain_bare"),
        name: "Bare".into(),
        description: None,
        color: None,
    }
}

#[test]
fn a_boundary_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_boundary(), &bare_boundary());
}

#[test]
fn a_topic_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_topic(), &bare_topic());
}

#[test]
fn a_question_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_question(), &bare_question());
}

#[test]
fn a_domain_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_domain(), &bare_domain());
}
