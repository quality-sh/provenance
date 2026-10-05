use super::seeded_requirement_store;
use crate::state_store::{
    CreateBoundaryInput, CreateDomainInput, CreateQuestionInput, CreateRequirementInput,
    CreateResolutionInput, CreateRuleInput, CreateSourceInput, CreateTopicInput, EditQuestionInput,
    UpdateBoundaryInput, UpdateDomainInput, UpdateResolutionInput, UpdateRuleInput,
    UpdateSourceInput, UpdateTopicInput,
};
use provenance_core::{
    NodeType, QuestionStatus, RequirementStatus, ResolutionMethod, ResolutionStatus, RuleSeverity,
    RuleStatus, SourceType, StableId, TopicStatus,
};

pub(super) fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

pub(super) fn seed_native_records(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
) {
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
}

/// The review revision of one native record.
fn revision(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    kind: NodeType,
    record: &str,
) -> Option<StableId> {
    store
        .record_edit_state(scope, kind, &id(record))
        .unwrap()
        .revision
}

const NATIVE_RECORDS: [(NodeType, &str); 7] = [
    (NodeType::Source, "source_native"),
    (NodeType::Domain, "domain_native"),
    (NodeType::Resolution, "resolution_native"),
    (NodeType::Rule, "rule_native"),
    (NodeType::Boundary, "boundary_native"),
    (NodeType::Topic, "topic_native"),
    (NodeType::Question, "question_native"),
];

#[test]
fn native_creators_enroll_every_other_record_kind() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);

    for (kind, record_id) in NATIVE_RECORDS {
        assert!(
            revision(&store, &scope, kind, record_id).is_some(),
            "{kind:?} is not enrolled"
        );
    }
}

fn update_native_records(store: &crate::state_store::StateStore, label: &str) {
    for value in [
        serde_json::json!({"scope_id":"default","id":"source_native","name":format!("Source {label}")}),
        serde_json::json!({"scope_id":"default","id":"resolution_native","position":format!("Use position {label}")}),
        serde_json::json!({"scope_id":"default","id":"rule_native","statement":format!("The system uses position {label}")}),
        serde_json::json!({"scope_id":"default","id":"domain_native","name":format!("Domain {label}")}),
        serde_json::json!({"scope_id":"default","id":"boundary_native","statement":format!("Boundary {label}")}),
        serde_json::json!({"scope_id":"default","id":"topic_native","title":format!("Topic {label}")}),
        serde_json::json!({"scope_id":"default","id":"question_native","question":format!("Is {label} correct?")}),
    ] {
        match value["id"].as_str().unwrap() {
            "source_native" => store
                .update_source(serde_json::from_value::<UpdateSourceInput>(value).unwrap())
                .map(|_| ()),
            "resolution_native" => store
                .update_resolution(serde_json::from_value::<UpdateResolutionInput>(value).unwrap())
                .map(|_| ()),
            "rule_native" => store
                .update_rule(serde_json::from_value::<UpdateRuleInput>(value).unwrap())
                .map(|_| ()),
            "domain_native" => store
                .update_domain(serde_json::from_value::<UpdateDomainInput>(value).unwrap())
                .map(|_| ()),
            "boundary_native" => store
                .update_boundary(serde_json::from_value::<UpdateBoundaryInput>(value).unwrap())
                .map(|_| ()),
            "topic_native" => store
                .edit_topic(serde_json::from_value::<UpdateTopicInput>(value).unwrap())
                .map(|_| ()),
            "question_native" => store
                .edit_question(serde_json::from_value::<EditQuestionInput>(value).unwrap())
                .map(|_| ()),
            _ => unreachable!(),
        }
        .unwrap();
    }
}

#[test]
#[provenance_macros::verifies("rule_review_revision_follows_review_content", examples)]
fn native_content_that_returns_to_an_earlier_value_returns_to_its_revision() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    let created = NATIVE_RECORDS.map(|(kind, record_id)| revision(&store, &scope, kind, record_id));

    update_native_records(&store, "B");
    let changed = NATIVE_RECORDS.map(|(kind, record_id)| revision(&store, &scope, kind, record_id));
    update_native_records(&store, "A");
    let returned =
        NATIVE_RECORDS.map(|(kind, record_id)| revision(&store, &scope, kind, record_id));

    for (index, (kind, _)) in NATIVE_RECORDS.iter().enumerate() {
        assert_ne!(changed[index], created[index], "{kind:?}");
        assert_eq!(returned[index], created[index], "{kind:?}");
    }
}

#[test]
fn source_rule_and_resolution_creators_refuse_unaddressed_origins() {
    let (_dir, store, scope) = seeded_requirement_store();
    let missing = id("message_missing");

    let source = CreateSourceInput {
        scope_id: scope.clone(),
        id: id("source_bad_origin"),
        name: "Source".into(),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: Some(missing.clone()),
    };
    let resolution = CreateResolutionInput {
        scope_id: scope.clone(),
        id: id("resolution_bad_origin"),
        title: "Resolution".into(),
        requirement_ids: vec![id("req_overtime")],
        supersedes: Vec::new(),
        position: "Use the position".into(),
        rationale: "The position satisfies the requirement".into(),
        status: ResolutionStatus::Proposed,
        context: None,
        enforcement: None,
        confidence: None,
        inputs: Vec::new(),
        made_by: None,
        approved_by: None,
        approved_at: None,
        origin_thread: None,
        origin_message: Some(missing.clone()),
    };
    let rule = CreateRuleInput {
        scope_id: scope.clone(),
        id: id("rule_bad_origin"),
        name: None,
        description: None,
        requirement_ids: vec![id("req_overtime")],
        resolution_ids: Vec::new(),
        statement: "The system refuses a missing origin".into(),
        status: RuleStatus::Active,
        archived_in_commit: None,
        severity: RuleSeverity::Medium,
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: Some(missing),
    };

    for result in [
        store.create_source(source).map(|_| ()),
        store.create_resolution(resolution).map(|_| ()),
        store.create_rule(rule).map(|_| ()),
    ] {
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("origin Message does not exist in this scope"));
    }
    assert_eq!(
        store.list_sources(&scope).unwrap(),
        [] as [provenance_core::Source; 0]
    );
    assert_eq!(
        store.list_resolutions(&scope).unwrap(),
        [] as [provenance_core::Resolution; 0]
    );
    assert_eq!(
        store.list_rules(&scope).unwrap(),
        [] as [provenance_core::Rule; 0]
    );
}

#[test]
#[provenance_macros::verifies("rule_review_revision_follows_review_content", examples)]
fn relation_and_shaping_writers_keep_the_revision_of_returned_content() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    let kept = [
        (NodeType::Source, "source_native"),
        (NodeType::Rule, "rule_native"),
        (NodeType::Resolution, "resolution_native"),
        (NodeType::Topic, "topic_native"),
    ];
    let created = kept.map(|(kind, record_id)| revision(&store, &scope, kind, record_id));
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: id("req_target"),
            statement: "A target requirement".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_source(CreateSourceInput {
            scope_id: scope.clone(),
            id: id("source_target"),
            name: "Target source".into(),
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
        .add_source_supersedes(&scope, &id("source_native"), id("source_target"))
        .unwrap();
    store
        .clear_source_supersedes(&scope, &id("source_native"), &id("source_target"))
        .unwrap();
    store
        .add_rule_requirement(&scope, &id("rule_native"), id("req_target"))
        .unwrap();
    store
        .clear_rule_requirement(&scope, &id("rule_native"), &id("req_target"))
        .unwrap();
    store
        .add_resolution_requirement(&scope, &id("resolution_native"), id("req_target"))
        .unwrap();
    store
        .clear_resolution_requirement(&scope, &id("resolution_native"), &id("req_target"))
        .unwrap();
    store
        .set_question_contradicts(&scope, &id("question_native"), id("req_target"))
        .unwrap();
    store
        .clear_question_contradicts(&scope, &id("question_native"))
        .unwrap();

    store
        .claim_topic(&scope, &id("topic_native"), "author")
        .unwrap();
    store.release_topic(&scope, &id("topic_native")).unwrap();
    store
        .claim_question(&scope, &id("question_native"), "author")
        .unwrap();
    store
        .release_question(&scope, &id("question_native"))
        .unwrap();
    store
        .answer_question(&scope, &id("question_native"), "A is correct".into(), None)
        .unwrap();

    for (index, (kind, record_id)) in kept.iter().enumerate() {
        assert_eq!(
            revision(&store, &scope, *kind, record_id),
            created[index],
            "{kind:?}"
        );
    }
    assert!(revision(&store, &scope, NodeType::Question, "question_native").is_some());
}

#[test]
fn graph_record_replacement_changes_the_revision() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    let created = revision(&store, &scope, NodeType::Source, "source_native");
    let mut sources = store.list_sources(&scope).unwrap();
    sources[0].name = "Source B".into();

    store
        .replace_graph_records(&crate::shards::sources_path(&store.layout, &scope), sources)
        .unwrap();

    assert_ne!(
        revision(&store, &scope, NodeType::Source, "source_native"),
        created
    );
}

#[test]
fn native_save_refuses_a_stale_etag_without_publishing_the_mutation() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    let stale_etag = store
        .record_edit_state(&scope, NodeType::Source, &id("source_native"))
        .unwrap()
        .etag;
    store
        .update_source(
            serde_json::from_value::<UpdateSourceInput>(serde_json::json!({
                "scope_id": "default",
                "id": "source_native",
                "name": "Source B"
            }))
            .unwrap(),
        )
        .unwrap();

    let error = store
        .update_source(
            serde_json::from_value::<UpdateSourceInput>(serde_json::json!({
                "scope_id": "default",
                "id": "source_native",
                "expected_etag": stale_etag,
                "name": "Source C"
            }))
            .unwrap(),
        )
        .unwrap_err();
    assert!(matches!(
        crate::write_error::WriteError(error).safe(),
        crate::write_error::WriteFailure::RecordEditConflict {
            record_kind: NodeType::Source,
            ..
        }
    ));
    assert_eq!(
        store
            .list_sources(&scope)
            .unwrap()
            .into_iter()
            .find(|source| source.id == id("source_native"))
            .unwrap()
            .name,
        "Source B"
    );
}
