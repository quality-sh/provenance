use super::*;
use crate::write_error::{WriteError, WriteFailure};

fn assert_enrolled_deletion(
    error: anyhow::Error,
    expected_kind: NodeType,
    expected_id: &StableId,
) {
    let error = WriteError(error);
    assert!(matches!(
        error.safe(),
        WriteFailure::EnrolledRecordDeletionConflict {
            record_kind,
            record_id,
        } if record_kind == expected_kind && record_id == *expected_id
    ));
    assert_eq!(error.status(), 409);
}

#[test]
fn typed_omission_of_only_an_enrolled_rule_returns_a_public_conflict() {
    let (_temp, store, scope) = fixture();
    let mut input = document(
        "Policy one",
        "The system stores records.",
        "The system retains records.",
    );
    store.apply_typed_spec(&scope, input.clone()).unwrap();
    let rule_id = store.list_rules(&scope).unwrap()[0].id.clone();
    enroll(&store, &scope, NodeType::Rule, &rule_id);
    input.rules.clear();

    let error = store.apply_typed_spec(&scope, input).unwrap_err();

    assert_enrolled_deletion(error, NodeType::Rule, &rule_id);
}

#[test]
fn typed_cascade_refuses_enrolled_deletion_before_publication() {
    let (_temp, store, scope) = fixture();
    store
        .apply_typed_spec(&scope, cascade_document(true))
        .unwrap();
    let removed = store
        .list_requirements(&scope)
        .unwrap()
        .into_iter()
        .find(|record| record.statement.contains("removes"))
        .unwrap()
        .id;
    let resolution_id = StableId::new("resolution_deleted_by_cascade").unwrap();
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: resolution_id.clone(),
            title: "Temporary record decision".into(),
            requirement_ids: vec![removed],
            supersedes: Vec::new(),
            position: "Remove the temporary record.".into(),
            rationale: "The record is temporary.".into(),
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
    enroll(&store, &scope, NodeType::Resolution, &resolution_id);
    let requirement_path = shards::requirements_path(&store.layout, &scope);
    let resolution_path = shards::resolutions_path(&store.layout, &scope);
    let requirements_before = std::fs::read(&requirement_path).unwrap();
    let resolutions_before = std::fs::read(&resolution_path).unwrap();

    let error = store
        .apply_typed_spec(&scope, cascade_document(false))
        .unwrap_err();

    assert_enrolled_deletion(error, NodeType::Resolution, &resolution_id);
    assert_eq!(
        std::fs::read(requirement_path).unwrap(),
        requirements_before
    );
    assert_eq!(std::fs::read(resolution_path).unwrap(), resolutions_before);
}

#[test]
fn typed_cascade_deletion_of_an_enrolled_topic_returns_a_public_conflict() {
    let (_temp, store, scope) = fixture();
    store
        .apply_typed_spec(&scope, cascade_document(true))
        .unwrap();
    let removed = store
        .list_requirements(&scope)
        .unwrap()
        .into_iter()
        .find(|record| record.statement.contains("removes"))
        .unwrap()
        .id;
    let topic_id = StableId::new("topic_deleted_by_cascade").unwrap();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: topic_id.clone(),
            requirement_id: removed,
            title: "Temporary storage".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    enroll(&store, &scope, NodeType::Topic, &topic_id);

    let error = store
        .apply_typed_spec(&scope, cascade_document(false))
        .unwrap_err();

    assert_enrolled_deletion(error, NodeType::Topic, &topic_id);
}
