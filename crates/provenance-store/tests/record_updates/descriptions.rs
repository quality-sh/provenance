use super::support::Fixture;
use serde_json::json;

#[tokio::test]
async fn requirement_statement_edit_raises_existing_reviews_and_keeps_relations() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture.call("create-rule", json!({"scope_id":"default","id":"rule_one","statement":"The system saves the record.","status":"draft","severity":"medium","requirement_ids":["req_one"],"resolution_ids":[]})).await.unwrap();
    let updated = fixture.call("update-requirement", json!({"scope_id":"default","id":"req_one","statement":"The system reads the record.","description":"Details"})).await.unwrap();
    assert_eq!(updated["statement"], "The system reads the record.");
    let reviews = fixture
        .store
        .open_requirement_reviews(&fixture.scope)
        .unwrap();
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].before, "The system saves the record.");
    assert_eq!(reviews[0].after, "The system reads the record.");
    let rule = fixture.call("update-rule", json!({"scope_id":"default","id":"rule_one","name":"Read","clear_fields":["description"],"status":"deprecated"})).await.unwrap();
    assert_eq!(rule["name"], "Read");
    assert_eq!(rule["requirement_ids"], json!(["req_one"]));
    assert_eq!(rule["status"], "deprecated");
}

#[tokio::test]
async fn descriptive_updates_keep_domain_boundary_topic_and_question_identity() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    for (kind, fields, patch, field) in [
        (
            "domain",
            json!({"name":"Domain","description":"Details","color":"blue"}),
            json!({"name":"Renamed","clear_fields":["description"]}),
            "name",
        ),
        (
            "boundary",
            json!({"requirement_id":"req_one","statement":"Limit"}),
            json!({"statement":"Renamed"}),
            "statement",
        ),
        (
            "topic",
            json!({"requirement_id":"req_one","title":"Topic","status":"open","links":[]}),
            json!({"title":"Renamed"}),
            "title",
        ),
        (
            "question",
            json!({"topic_id":"topic_one","question":"Question?","resolution_method":"research","status":"open","links":[]}),
            json!({"question":"Renamed"}),
            "question",
        ),
    ] {
        let mut fields = fields;
        fields["id"] = json!(format!("{kind}_one"));
        fields["scope_id"] = json!("default");
        let before = fixture
            .call(&format!("create-{kind}"), fields)
            .await
            .unwrap();
        let mut patch = patch;
        patch["id"] = before["id"].clone();
        patch["scope_id"] = json!("default");
        let after = fixture
            .call(&format!("update-{kind}"), patch)
            .await
            .unwrap();
        assert_eq!(after["id"], before["id"]);
        assert_eq!(after[field], "Renamed");
        assert_eq!(after["requirement_id"], before["requirement_id"]);
        assert_eq!(after["topic_id"], before["topic_id"]);
    }
}

#[tokio::test]
async fn invalid_requirement_edits_leave_statements_and_reviews_unchanged() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let before = fixture.store.list_requirements(&fixture.scope).unwrap();
    for fields in [
        json!({"statement":"The system reads the record.","domain_id":"domain_missing"}),
        json!({"description":"conflict","clear_fields":["description"]}),
        json!({"statement":"Stop; wait."}),
        json!({"clear_fields":["statement"]}),
    ] {
        let mut request = fields;
        request["scope_id"] = json!("default");
        request["id"] = json!("req_one");
        assert!(fixture.call("update-requirement", request).await.is_err());
        assert_eq!(
            fixture.store.list_requirements(&fixture.scope).unwrap(),
            before
        );
        assert!(fixture
            .store
            .open_requirement_reviews(&fixture.scope)
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn owned_requirement_and_rule_updates_preserve_declaration_identity() {
    let fixture = Fixture::new();
    let spec = json!({"schema_version":2,"spec":"fixture","declared_by":"spec://fixture",
        "requirements":[{"key":"ready","statement":"The system is ready."}],
        "rules":[{"key":"ready","requirement":"ready","statement":"The system is ready."}]});
    fixture.call("apply", spec.clone()).await.unwrap();
    let requirement =
        serde_json::to_value(&fixture.store.list_requirements(&fixture.scope).unwrap()[0]).unwrap();
    let rule = serde_json::to_value(&fixture.store.list_rules(&fixture.scope).unwrap()[0]).unwrap();
    for (operation, before) in [("update-requirement", requirement), ("update-rule", rule)] {
        for owner in [None, Some("spec://other")] {
            let mut request =
                json!({"scope_id":"default","id":before["id"],"description":"Changed"});
            if let Some(owner) = owner {
                request["declared_by"] = json!(owner);
            }
            let error = fixture.call(operation, request).await.unwrap_err();
            assert_eq!(error["kind"], "record_ownership_conflict");
        }
        let after = fixture.call(operation, json!({"scope_id":"default","id":before["id"],"declared_by":"spec://fixture","description":"Changed","retired":true})).await.unwrap();
        let mut expected = before;
        expected["description"] = json!("Changed");
        expected["retired"] = json!(true);
        assert_eq!(after, expected);
    }
    fixture.call("apply", spec).await.unwrap();
    assert!(!fixture.store.list_requirements(&fixture.scope).unwrap()[0].retired);
    assert!(!fixture.store.list_rules(&fixture.scope).unwrap()[0].retired);
    assert!(fixture
        .store
        .open_requirement_reviews(&fixture.scope)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn metadata_only_and_unchanged_statements_do_not_create_reviews() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture.call("create-rule", json!({"scope_id":"default","id":"rule_one","statement":"The system saves the record.","status":"draft","severity":"medium","requirement_ids":["req_one"],"resolution_ids":[]})).await.unwrap();
    fixture.call("update-requirement", json!({"scope_id":"default","id":"req_one","description":"Details","statement":"The system saves the record."})).await.unwrap();
    assert!(fixture
        .store
        .open_requirement_reviews(&fixture.scope)
        .unwrap()
        .is_empty());
}
