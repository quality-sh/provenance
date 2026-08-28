use super::{seeded_requirement_store, seeded_source_requirement_store};
use crate::state_store::{
    CreateBoundaryInput, CreateQuestionInput, CreateTopicInput, UpdateQuestionInput,
};
use provenance_core::{
    ArtifactLink, ArtifactLinkTargetType, QuestionStatus, ResolutionMethod, SourceReference,
    StableId, TopicStatus,
};
use provenance_macros::verifies;

#[test]
fn shaping_records_are_written_deterministically_and_validate_relationships() {
    let (_dir, store, scope) = seeded_source_requirement_store();

    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: StableId::new("topic_b").unwrap(),
            requirement_id: StableId::new("req_overtime").unwrap(),
            title: "B topic".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: StableId::new("topic_a").unwrap(),
            requirement_id: StableId::new("req_overtime").unwrap(),
            title: "A topic".into(),
            status: TopicStatus::Explored,
            links: vec![
                ArtifactLink {
                    target_type: ArtifactLinkTargetType::Source,
                    target_id: StableId::new("source_schads").unwrap(),
                },
                ArtifactLink {
                    target_type: ArtifactLinkTargetType::Requirement,
                    target_id: StableId::new("req_overtime").unwrap(),
                },
                ArtifactLink {
                    target_type: ArtifactLinkTargetType::Source,
                    target_id: StableId::new("source_schads").unwrap(),
                },
            ],
        })
        .unwrap();
    store
        .create_boundary(CreateBoundaryInput {
            scope_id: scope.clone(),
            id: StableId::new("boundary_no_manual_rework").unwrap(),
            requirement_id: StableId::new("req_overtime").unwrap(),
            statement: "No manual rework".into(),
            source_ref: Some(SourceReference {
                source_id: StableId::new("source_schads").unwrap(),
                clause: Some("28.1".into()),
            }),
        })
        .unwrap();
    let question = store
        .create_question(CreateQuestionInput {
            scope_id: scope.clone(),
            id: StableId::new("question_threshold").unwrap(),
            topic_id: StableId::new("topic_a").unwrap(),
            question: "Which threshold applies?".into(),
            resolution_method: ResolutionMethod::Grill,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
        })
        .unwrap();

    let topics = store.list_topics(&scope).unwrap();
    assert_eq!(topics[0].id.as_str(), "topic_a");
    assert_eq!(topics[0].links.len(), 2);
    assert_eq!(topics[0].links[0].target_id.as_str(), "req_overtime");
    assert_eq!(topics[0].links[1].target_id.as_str(), "source_schads");
    assert_eq!(
        store.list_boundaries(&scope).unwrap()[0]
            .source_ref
            .as_ref()
            .unwrap()
            .clause
            .as_deref(),
        Some("28.1")
    );
    assert_eq!(question.requirement_id.as_str(), "req_overtime");
    assert!(store
        .create_question(CreateQuestionInput {
            scope_id: scope,
            id: StableId::new("question_missing_topic").unwrap(),
            topic_id: StableId::new("topic_missing").unwrap(),
            question: "Missing topic?".into(),
            resolution_method: ResolutionMethod::Grill,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
        })
        .unwrap_err()
        .to_string()
        .contains("topic does not exist"));
}

#[test]
#[verifies("rule_claim_eligibility", examples)]
#[verifies("rule_claim_cleared_on_exit", examples)]
fn topic_claims_are_check_and_set_and_clear_on_close() {
    let (_dir, store, scope) = seeded_requirement_store();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: StableId::new("topic_overtime").unwrap(),
            requirement_id: StableId::new("req_overtime").unwrap(),
            title: "Overtime eligibility".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    let topic_id = StableId::new("topic_overtime").unwrap();

    let claimed = store
        .claim_topic(&scope, &topic_id, "agent-one", Vec::<String>::new())
        .unwrap();
    assert_eq!(claimed.topic.claimed_by.as_deref(), Some("agent-one"));
    assert!(claimed.topic.claimed_at.unwrap() > 0);

    let err = store
        .claim_topic(&scope, &topic_id, "agent-two", Vec::<String>::new())
        .unwrap_err();
    assert!(err
        .to_string()
        .contains("topic topic_overtime is already claimed by agent-one"));

    let released = store.release_topic(&scope, &topic_id).unwrap();
    assert_eq!(released.claimed_by, None);
    assert_eq!(released.claimed_at, None);
    assert!(store
        .release_topic(&scope, &topic_id)
        .unwrap_err()
        .to_string()
        .contains("topic topic_overtime is not claimed"));

    store
        .claim_topic(&scope, &topic_id, "agent-two", Vec::<String>::new())
        .unwrap();
    let closed = store.close_topic(&scope, &topic_id).unwrap();
    assert_eq!(closed.status, TopicStatus::Closed);
    assert_eq!(closed.claimed_by, None);
    assert_eq!(closed.claimed_at, None);
    assert!(store
        .claim_topic(&scope, &topic_id, "agent-one", Vec::<String>::new())
        .unwrap_err()
        .to_string()
        .contains("closed"));
    assert_eq!(
        store.list_topics(&scope).unwrap()[0].status,
        TopicStatus::Closed
    );
}

#[test]
#[verifies("rule_claim_eligibility", examples)]
#[verifies("rule_claim_cleared_on_exit", examples)]
fn question_claims_clear_when_answered() {
    let (_dir, store, scope) = seeded_requirement_store();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: StableId::new("topic_overtime").unwrap(),
            requirement_id: StableId::new("req_overtime").unwrap(),
            title: "Overtime eligibility".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            scope_id: scope.clone(),
            id: StableId::new("question_threshold").unwrap(),
            topic_id: StableId::new("topic_overtime").unwrap(),
            question: "Which threshold applies?".into(),
            resolution_method: ResolutionMethod::Research,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
        })
        .unwrap();
    let question_id = StableId::new("question_threshold").unwrap();

    let claimed = store
        .claim_question(&scope, &question_id, "agent-one")
        .unwrap();
    assert_eq!(claimed.claimed_by.as_deref(), Some("agent-one"));
    assert_eq!(claimed.resolution_method, ResolutionMethod::Research);
    assert!(store
        .claim_question(&scope, &question_id, "agent-two")
        .unwrap_err()
        .to_string()
        .contains("question question_threshold is already claimed by agent-one"));

    let answered = store
        .answer_question(
            &scope,
            &question_id,
            "Use the SCHADS threshold.".into(),
            None,
        )
        .unwrap();
    assert_eq!(answered.status, QuestionStatus::Answered);
    assert_eq!(
        answered.answer.as_deref(),
        Some("Use the SCHADS threshold.")
    );
    assert_eq!(answered.claimed_by, None);
    assert_eq!(answered.claimed_at, None);
    assert!(store
        .claim_question(&scope, &question_id, "agent-two")
        .unwrap_err()
        .to_string()
        .contains("answered"));

    let persisted = &store.list_questions(&scope).unwrap()[0];
    assert_eq!(persisted.status, QuestionStatus::Answered);
    assert_eq!(persisted.claimed_by, None);
}

#[test]
#[verifies("rule_claim_cleared_on_exit", examples)]
fn question_claims_clear_when_an_update_blocks_them_on_a_human() {
    let (_dir, store, scope) = seeded_requirement_store();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: StableId::new("topic_overtime").unwrap(),
            requirement_id: StableId::new("req_overtime").unwrap(),
            title: "Overtime eligibility".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            scope_id: scope.clone(),
            id: StableId::new("question_threshold").unwrap(),
            topic_id: StableId::new("topic_overtime").unwrap(),
            question: "Which threshold applies?".into(),
            resolution_method: ResolutionMethod::Research,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
        })
        .unwrap();
    let question_id = StableId::new("question_threshold").unwrap();
    store
        .claim_question(&scope, &question_id, "agent-one")
        .unwrap();

    let retargeted = store
        .update_question(UpdateQuestionInput {
            scope_id: scope.clone(),
            id: question_id.clone(),
            resolution_method: Some(ResolutionMethod::Grill),
            status: None,
            links: None,
            resolution_id: None,
        })
        .unwrap();
    assert_eq!(retargeted.claimed_by.as_deref(), Some("agent-one"));

    let blocked = store
        .update_question(UpdateQuestionInput {
            scope_id: scope.clone(),
            id: question_id.clone(),
            resolution_method: None,
            status: Some(QuestionStatus::BlockedOnHuman),
            links: None,
            resolution_id: None,
        })
        .unwrap();
    assert_eq!(blocked.status, QuestionStatus::BlockedOnHuman);
    assert_eq!(blocked.claimed_by, None);
    assert_eq!(blocked.claimed_at, None);
    assert!(store
        .claim_question(&scope, &question_id, "agent-two")
        .unwrap_err()
        .to_string()
        .contains("blocked_on_human"));
}
