use super::support::Fixture;
use serde_json::json;

#[tokio::test]
async fn source_update_preserves_omissions_and_clears_nullable_fields() {
    let fixture = Fixture::new();
    let mut expected = fixture.source().await;
    expected["url"] = json!("https://new.example");
    expected.as_object_mut().unwrap().remove("reference");
    expected.as_object_mut().unwrap().remove("commit_pin");
    let actual = fixture.call("update-source", json!({"scope_id":"default","id":"source_one","url":"https://new.example","clear_fields":["reference","commit_pin"]})).await.unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        serde_json::to_value(&fixture.store.list_sources(&fixture.scope).unwrap()[0]).unwrap(),
        expected
    );
}

#[tokio::test]
async fn source_update_refusals_do_not_publish_any_field() {
    let fixture = Fixture::new();
    let before = fixture.source().await;
    for patch in [
        json!({"id":"source_missing","url":"changed"}),
        json!({"id":"source_one","url":"changed","commit_pin":"bad"}),
        json!({"id":"source_one","url":"changed","clear_fields":["name"]}),
        json!({"id":"source_one","source_type":"unknown"}),
        json!({"id":"source_one","declared_by":"forged"}),
    ] {
        let mut patch = patch;
        patch["scope_id"] = json!("default");
        let error = fixture.call("update-source", patch).await.unwrap_err();
        assert_ne!(error["kind"], "unknown_operation");
        assert_eq!(
            serde_json::to_value(&fixture.store.list_sources(&fixture.scope).unwrap()[0]).unwrap(),
            before
        );
    }
}

#[tokio::test]
async fn direct_updates_do_not_take_over_typed_declarations() {
    let fixture = Fixture::new();
    fixture.call("apply", json!({"schema_version":2,"spec":"fixture","declared_by":"spec://fixture","sources":[{"key":"policy","name":"Policy","kind":"policy"}],"requirements":[]})).await.unwrap();
    let before = fixture.store.list_sources(&fixture.scope).unwrap();
    let error = fixture
        .call(
            "update-source",
            json!({"scope_id":"default","id":before[0].id,"name":"Taken over"}),
        )
        .await
        .unwrap_err();
    assert_ne!(error["kind"], "unknown_operation");
    assert_eq!(fixture.store.list_sources(&fixture.scope).unwrap(), before);
}

#[tokio::test]
async fn matching_owner_can_edit_metadata_without_moving_or_adopting_the_record() {
    let fixture = Fixture::new();
    fixture.call("apply", json!({"schema_version":2,"spec":"fixture","declared_by":"spec://fixture","sources":[{"key":"policy","name":"Policy","kind":"policy"}],"requirements":[]})).await.unwrap();
    let before = fixture
        .store
        .list_sources(&fixture.scope)
        .unwrap()
        .remove(0);
    for owner in [None, Some("spec://other")] {
        let mut request = json!({"scope_id":"default","id":before.id,"name":"Wrong","commit_pin":"abcdef0123456789"});
        if let Some(owner) = owner {
            request["declared_by"] = json!(owner);
        }
        let error = fixture.call("update-source", request).await.unwrap_err();
        assert_eq!(error["kind"], "record_ownership_conflict");
        assert_eq!(
            fixture.store.list_sources(&fixture.scope).unwrap(),
            vec![before.clone()]
        );
    }
    let updated = fixture.call("update-source", json!({"scope_id":"default","id":before.id,"declared_by":"spec://fixture","commit_pin":"abcdef0123456789","effective_date":1234,"retired":true})).await.unwrap();
    assert_eq!(updated["id"], json!(before.id));
    assert_eq!(
        updated["declaration_address"],
        json!(before.declaration_address)
    );
    assert_eq!(updated["declared_by"], "spec://fixture");
    assert_eq!(updated["retired"], true);
    assert_eq!(updated["commit_pin"], "abcdef0123456789");
    assert_eq!(fixture.store.list_sources(&fixture.scope).unwrap().len(), 1);
    fixture.call("apply", json!({"schema_version":2,"spec":"fixture","declared_by":"spec://fixture","sources":[{"key":"policy","name":"Policy after apply","kind":"policy"}],"requirements":[]})).await.unwrap();
    let reapplied = fixture
        .store
        .list_sources(&fixture.scope)
        .unwrap()
        .remove(0);
    assert_eq!(reapplied.id, before.id);
    assert_eq!(reapplied.declaration_address, before.declaration_address);
    assert_eq!(reapplied.name, "Policy after apply");
    assert_eq!(reapplied.commit_pin.as_deref(), Some("abcdef0123456789"));
    assert!(!reapplied.retired);
}

#[tokio::test]
async fn concurrent_partial_updates_keep_both_changes() {
    let fixture = Fixture::new();
    fixture.source().await;
    std::thread::scope(|scope| {
        let store = &fixture.store;
        scope.spawn(move || {
            store
                .update_source(
                    serde_json::from_value(
                        json!({"scope_id":"default","id":"source_one","url":"https://new.example"}),
                    )
                    .unwrap(),
                )
                .unwrap()
        });
        scope.spawn(move || {
            store
                .update_source(
                    serde_json::from_value(
                        json!({"scope_id":"default","id":"source_one","reference":"section 2"}),
                    )
                    .unwrap(),
                )
                .unwrap()
        });
    });
    let saved = fixture
        .store
        .list_sources(&fixture.scope)
        .unwrap()
        .remove(0);
    assert_eq!(saved.url.as_deref(), Some("https://new.example"));
    assert_eq!(saved.reference.as_deref(), Some("section 2"));
}

#[tokio::test]
async fn null_is_omission_and_invalid_clear_requests_are_atomic() {
    let fixture = Fixture::new();
    let before = fixture.source().await;
    let unchanged = fixture
        .call(
            "update-source",
            json!({"scope_id":"default","id":"source_one","name":null,"url":null}),
        )
        .await
        .unwrap();
    assert_eq!(unchanged, before);
    for fields in [
        json!({"name":" "}),
        json!({"url":"changed","clear_fields":["url"]}),
        json!({"unknown":"value"}),
        json!({"scope_id":"other","url":"changed"}),
    ] {
        let mut request = json!({"scope_id":"default","id":"source_one"});
        request
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        assert!(fixture.call("update-source", request).await.is_err());
        assert_eq!(
            serde_json::to_value(&fixture.store.list_sources(&fixture.scope).unwrap()[0]).unwrap(),
            before
        );
    }
}
