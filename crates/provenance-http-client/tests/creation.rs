use provenance_http_client::{Error, HttpClient};
use serde_json::{json, Value};

#[tokio::test]
#[ignore = "run through node tools/operation-codegen/test-clients.mjs creation"]
async fn generated_creation_methods_preserve_records_and_refusals() {
    let fixture: Value =
        serde_json::from_str(&std::env::var("PROVENANCE_RECORDS_FIXTURE").unwrap()).unwrap();
    let client =
        HttpClient::connect_with_bearer(fixture["url"].as_str().unwrap(), "fixture-secret")
            .await
            .unwrap();
    let context = json!({"repository":"fixture","scope":"default"});
    let source = json!({"context":context,"request":{"scope_id":"default","id":"source_rs","name":"Policy","source_type":"policy","supersedes":[],"origin_thread":"thread_origin","origin_message":"message_origin"}});
    let created = client
        .create_source(&serde_json::from_value(source.clone()).unwrap())
        .await
        .unwrap();
    let created = serde_json::to_value(created).unwrap();
    assert_eq!(created["origin_thread"], "thread_origin");
    assert_eq!(created["origin_message"], "message_origin");
    let requirement = client.create_requirement(&serde_json::from_value(json!({"context":context,"request":{"scope_id":"default","id":"req_rs","statement":"The system is ready.","status":"discovery","depends_on":[],"supersedes":[]}})).unwrap()).await.unwrap();
    assert_eq!(
        serde_json::to_value(requirement).unwrap()["status"],
        "discovery"
    );
    let resolution = client.create_resolution(&serde_json::from_value(json!({"context":context,"request":{"scope_id":"default","id":"res_rs","title":"Decision","requirement_ids":["req_rs"],"supersedes":[],"position":"Use the existing record.","rationale":"The shape is fixed.","status":"draft","inputs":[]}})).unwrap()).await.unwrap();
    assert_eq!(
        serde_json::to_value(resolution).unwrap()["requirement_ids"],
        json!(["req_rs"])
    );
    let rule = client.create_rule(&serde_json::from_value(json!({"context":context,"request":{"scope_id":"default","id":"rule_rs","statement":"The system is ready.","requirement_ids":["req_rs"],"resolution_ids":["res_rs"],"status":"draft","severity":"high"}})).unwrap()).await.unwrap();
    assert_eq!(
        serde_json::to_value(rule).unwrap()["resolution_ids"],
        json!(["res_rs"])
    );
    let linked = client.add_source_reference(&serde_json::from_value(json!({"context":context,"request":{"scope_id":"default","source_id":"source_rs","requirement_id":"req_rs","clause":"1"}})).unwrap()).await.unwrap();
    assert_eq!(
        serde_json::to_value(linked).unwrap()["source_refs"],
        json!([{"source_id":"source_rs","clause":"1"}])
    );
    let changed = client.update_source(&serde_json::from_value(json!({"context":context,"request":{"scope_id":"default","id":"source_rs","url":"https://example.test/new","reference":"section 2","commit_pin":"abcdef0123456789"}})).unwrap()).await.unwrap();
    assert_eq!(
        serde_json::to_value(changed).unwrap()["origin_thread"],
        "thread_origin"
    );
    let cleared = client.update_source(&serde_json::from_value(json!({"context":context,"request":{"scope_id":"default","id":"source_rs","clear_fields":["reference","commit_pin"]}})).unwrap()).await.unwrap();
    let cleared = serde_json::to_value(cleared).unwrap();
    assert_eq!(cleared["url"], "https://example.test/new");
    assert!(cleared.get("reference").is_none());
    assert!(cleared.get("commit_pin").is_none());
    let approved = client.update_resolution(&serde_json::from_value(json!({"context":context,"request":{"scope_id":"default","id":"res_rs","status":"approved","approved_by":"reviewer","approved_at":1234,"inputs":[{"input_type":"source_material","reference":"source_rs","summary":"Later evidence"}]}})).unwrap()).await.unwrap();
    let approved = serde_json::to_value(approved).unwrap();
    assert_eq!(approved["approved_at"], 1234);
    assert_eq!(approved["approved_by"], "reviewer");
    assert_eq!(approved["requirement_ids"], json!(["req_rs"]));
    match client
        .create_source(&serde_json::from_value(source).unwrap())
        .await
        .unwrap_err()
    {
        Error::Operation { status, failure } => {
            assert_eq!(status, 409);
            assert_eq!(
                serde_json::to_value(failure).unwrap()["error"]["kind"],
                "already_exists"
            );
        }
        error => panic!("expected typed refusal, got {error:?}"),
    }
}
