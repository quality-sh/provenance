use super::seeded_requirement_store;
use crate::state_store::{
    CreateBoundaryInput, CreateDomainInput, CreateQuestionInput, CreateResolutionInput,
    CreateRuleInput, CreateSourceInput, CreateTopicInput,
};
use provenance_core::review::SaveOutcome;
use provenance_core::{
    NodeType, QuestionStatus, ResolutionMethod, ResolutionStatus, RuleSeverity, RuleStatus,
    SourceType, StableId, TopicStatus,
};

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

#[test]
fn native_creators_capture_created_occurrences_for_every_other_record_kind() {
    let (_dir, store, scope) = seeded_requirement_store();

    store
        .create_source(CreateSourceInput {
            scope_id: scope.clone(),
            id: id("source_native"),
            name: "Source A".into(),
            source_type: SourceType::Policy,
            url: None,
            reference: None,
            commit_pin: None,
            effective_date: None,
            review_date: None,
            supersedes: Vec::new(),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_domain(CreateDomainInput {
            scope_id: scope.clone(),
            id: id("domain_native"),
            name: "Domain A".into(),
            description: None,
            color: None,
        })
        .unwrap();
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: id("resolution_native"),
            title: "Resolution A".into(),
            requirement_ids: vec![id("req_overtime")],
            supersedes: Vec::new(),
            position: "Use position A".into(),
            rationale: "Position A satisfies the requirement".into(),
            status: ResolutionStatus::Proposed,
            context: None,
            enforcement: None,
            confidence: None,
            inputs: Vec::new(),
            made_by: None,
            approved_by: None,
            approved_at: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: id("rule_native"),
            name: None,
            description: None,
            requirement_ids: vec![id("req_overtime")],
            resolution_ids: vec![id("resolution_native")],
            statement: "The system uses position A".into(),
            status: RuleStatus::Active,
            archived_in_commit: None,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_boundary(CreateBoundaryInput {
            scope_id: scope.clone(),
            id: id("boundary_native"),
            requirement_id: id("req_overtime"),
            statement: "Boundary A".into(),
            source_ref: None,
        })
        .unwrap();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: id("topic_native"),
            requirement_id: id("req_overtime"),
            title: "Topic A".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            scope_id: scope.clone(),
            id: id("question_native"),
            topic_id: id("topic_native"),
            question: "Is A correct?".into(),
            resolution_method: ResolutionMethod::Research,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
            contradicts: None,
        })
        .unwrap();

    let entries = store.review_entries(&scope).unwrap();
    for (kind, record_id) in [
        (NodeType::Source, "source_native"),
        (NodeType::Domain, "domain_native"),
        (NodeType::Resolution, "resolution_native"),
        (NodeType::Rule, "rule_native"),
        (NodeType::Boundary, "boundary_native"),
        (NodeType::Topic, "topic_native"),
        (NodeType::Question, "question_native"),
    ] {
        let matches = entries
            .iter()
            .filter(|entry| entry.record_kind == kind && entry.record_id.as_str() == record_id)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "missing creation occurrence for {kind}");
        assert_eq!(matches[0].outcome, SaveOutcome::Created);
        assert!(matches[0].before.is_none());
    }
}
