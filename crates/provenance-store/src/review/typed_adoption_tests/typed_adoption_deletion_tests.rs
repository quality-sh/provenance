use super::*;

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

    assert!(
        error
            .to_string()
            .contains("cannot delete enrolled resolution"),
        "{error:#}"
    );
    assert_eq!(std::fs::read(requirement_path).unwrap(), requirements_before);
    assert_eq!(std::fs::read(resolution_path).unwrap(), resolutions_before);
}
