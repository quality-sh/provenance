use super::shaping::{
    ArtifactLinkTargetType, Boundary, Question, QuestionStatus, ResolutionMethod, Topic,
    TopicStatus,
};

#[test]
fn shaping_records_roundtrip_from_convex_style_json() {
    let boundary = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "boundary_no_manual_payroll",
        "requirementId": "req_payroll",
        "statement": "Do not require manual payroll reconciliation",
        "sourceRef": {"sourceId": "source_schads", "clause": "28.1"}
    });
    let topic = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "topic_overtime",
        "requirementId": "req_payroll",
        "title": "Overtime eligibility",
        "status": "explored",
        "links": [{"targetType": "source", "targetId": "source_schads"}]
    });
    let question = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "question_overtime_threshold",
        "topicId": "topic_overtime",
        "requirementId": "req_payroll",
        "question": "Which threshold applies?",
        "resolutionMethod": "research",
        "status": "answered",
        "claimedBy": "agent-shaper",
        "claimedAt": 1_714_780_800_000_i64,
        "answer": "Use the SCHADS overtime threshold.",
        "links": [{"targetType": "resolution", "targetId": "res_overtime"}],
        "resolutionId": "res_overtime"
    });

    let boundary: Boundary = serde_json::from_value(boundary).unwrap();
    let topic: Topic = serde_json::from_value(topic).unwrap();
    let question: Question = serde_json::from_value(question).unwrap();

    assert_eq!(boundary.requirement_id.as_str(), "req_payroll");
    assert_eq!(
        boundary.source_ref.as_ref().unwrap().source_id.as_str(),
        "source_schads"
    );
    assert_eq!(topic.status, TopicStatus::Explored);
    assert_eq!(topic.links[0].target_type, ArtifactLinkTargetType::Source);
    assert_eq!(topic.claimed_by, None);
    assert_eq!(question.topic_id.as_str(), "topic_overtime");
    assert_eq!(question.resolution_method, ResolutionMethod::Research);
    assert_eq!(question.claimed_by.as_deref(), Some("agent-shaper"));
    assert_eq!(question.claimed_at, Some(1_714_780_800_000));
    assert_eq!(question.status, QuestionStatus::Answered);
    assert_eq!(
        question.resolution_id.as_ref().unwrap().as_str(),
        "res_overtime"
    );

    let boundary = serde_json::to_value(boundary).unwrap();
    let topic = serde_json::to_value(topic).unwrap();
    let question = serde_json::to_value(question).unwrap();

    assert_eq!(boundary["schema_version"], SUPPORTED_SCHEMA_VERSION.0);
    assert_eq!(boundary["requirement_id"], "req_payroll");
    assert_eq!(boundary["source_ref"]["source_id"], "source_schads");
    assert_eq!(topic["status"], "explored");
    assert_eq!(topic["links"][0]["target_type"], "source");
    assert!(topic.get("claimed_by").is_none());
    assert!(topic.get("claimed_at").is_none());
    assert_eq!(question["resolution_method"], "research");
    assert_eq!(question["status"], "answered");
    assert_eq!(question["claimed_by"], "agent-shaper");
    assert_eq!(question["claimed_at"], 1_714_780_800_000_i64);
    assert_eq!(question["resolution_id"], "res_overtime");
}

#[test]
fn question_blocked_on_human_status_accepts_hyphenated_state_and_roundtrips() {
    let question = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "question_fork",
        "topicId": "topic_overtime",
        "requirementId": "req_overtime",
        "question": "Which UI direction should the shaping map use?",
        "resolutionMethod": "prototype",
        "status": "blocked-on-human",
        "links": []
    });

    let question: Question = serde_json::from_value(question).unwrap();
    let question = serde_json::to_value(question).unwrap();

    assert_eq!(question["status"], "blocked_on_human");
    assert_eq!(question["resolution_method"], "prototype");
}
