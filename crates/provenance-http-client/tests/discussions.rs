use provenance_http_client::{Error, HttpClient};
use serde_json::{json, Value};

#[tokio::test]
#[ignore = "run through node tools/operation-codegen/test-clients.mjs discussions"]
async fn generated_discussion_methods_match_persisted_records() {
    let fixture: Value =
        serde_json::from_str(&std::env::var("PROVENANCE_RECORDS_FIXTURE").unwrap()).unwrap();
    let client =
        HttpClient::connect_with_bearer(fixture["url"].as_str().unwrap(), "fixture-secret")
            .await
            .unwrap();
    let context = json!({"repository":"fixture","scope":"default"});
    let request = json!({"scope_id":"default","parent":{"node_type":"topic","node_id":"topic_rs"},"role":"assistant","body":" Rust text "});
    let created = client
        .post_thread_message(
            &serde_json::from_value(json!({"context":context,"request":request})).unwrap(),
        )
        .await
        .unwrap();
    let created = json!(created);
    assert_eq!(created["message"]["body"], " Rust text ");
    let lists = json!({"context":context,"request":null});
    let threads = client
        .list_threads(&serde_json::from_value(lists.clone()).unwrap())
        .await
        .unwrap();
    let messages = client
        .list_messages(&serde_json::from_value(lists).unwrap())
        .await
        .unwrap();
    let read = |file: &str| -> Vec<Value> {
        let path = std::path::Path::new(fixture["root"].as_str().unwrap())
            .join(".provenance/state/scopes/default/threads")
            .join(file);
        std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    };
    assert_eq!(json!(threads), json!(read("threads.jsonl")));
    assert_eq!(json!(messages), json!(read("2026-07.jsonl")));
    assert!(json!(messages)
        .as_array()
        .unwrap()
        .contains(&created["message"]));
    for (field, value, kind) in [
        ("body", json!(" "), "empty_message_body"),
        ("scope_id", json!("other"), "scope_mismatch"),
        (
            "parent",
            json!({"node_type":"boundary","node_id":"absent"}),
            "unsupported_thread_parent",
        ),
    ] {
        let mut input = request.clone();
        input[field] = value;
        match client
            .post_thread_message(
                &serde_json::from_value(json!({"context":context,"request":input})).unwrap(),
            )
            .await
            .unwrap_err()
        {
            Error::Operation { status, failure } => {
                assert_eq!(status, 400);
                assert_eq!(json!(failure)["error"]["kind"], kind);
            }
            error => panic!("expected typed refusal, got {error:?}"),
        }
    }
}
