use super::support::Fixture;
use serde_json::json;

#[tokio::test]
async fn resolution_update_retains_placement_and_records_approval_and_later_input() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let mut expected = fixture.call("create-resolution", json!({"scope_id":"default","id":"resolution_one","title":"Decision","position":"Use the saved record.","rationale":"The record is available.","status":"draft","requirement_ids":["req_one"],"supersedes":[],"inputs":[]})).await.unwrap();
    let inputs = json!([{"input_type":"source_material","reference":"source_one","summary":"Later evidence"}]);
    expected["status"] = json!("approved");
    expected["approved_by"] = json!("reviewer");
    expected["approved_at"] = json!(1234);
    expected["inputs"] = inputs.clone();
    let actual = fixture.call("update-resolution", json!({"scope_id":"default","id":"resolution_one","status":"approved","approved_by":"reviewer","approved_at":1234,"inputs":inputs})).await.unwrap();
    assert_eq!(actual, expected);
    let before = fixture.store.list_resolutions(&fixture.scope).unwrap();
    let error = fixture
        .call(
            "update-resolution",
            json!({"scope_id":"default","id":"resolution_one","title":"changed","confidence":2.0}),
        )
        .await
        .unwrap_err();
    assert_ne!(error["kind"], "unknown_operation");
    assert_eq!(
        fixture.store.list_resolutions(&fixture.scope).unwrap(),
        before
    );
}

#[tokio::test]
async fn resolution_input_refusals_preserve_existing_approval_and_text() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let before = fixture.call("create-resolution", json!({"scope_id":"default","id":"resolution_one","title":"Decision","position":"Use the saved record.","rationale":"The record is available.","status":"draft","requirement_ids":["req_one"],"supersedes":[],"inputs":[]})).await.unwrap();
    for patch in [
        json!({"id":"missing","title":"Changed"}),
        json!({"title":" ","approved_by":"reviewer"}),
        json!({"approved_by":"reviewer","clear_fields":["approved_by"]}),
        json!({"title":"Changed","inputs":[{"input_type":"source_material","reference":"","summary":""}]}),
    ] {
        let mut request = json!({"scope_id":"default","id":"resolution_one"});
        request
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(fixture.call("update-resolution", request).await.is_err());
        assert_eq!(
            serde_json::to_value(&fixture.store.list_resolutions(&fixture.scope).unwrap()[0])
                .unwrap(),
            before
        );
    }
}
