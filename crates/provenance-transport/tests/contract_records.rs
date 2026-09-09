#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
mod records;
use records::{call, get_call, host, Repository};
use serde_json::json;

#[tokio::test]
async fn configured_target_reads_its_saved_uncommitted_graph() {
    let first = Repository::new("The first value is selected.");
    let second = Repository::new("The second value is selected.");
    let current = std::env::current_dir().unwrap();
    assert_ne!(current, first.dir.path());
    assert_ne!(current, second.dir.path());
    let host = host(
        &[("first", &first), ("second", &second)],
        &["first", "second"],
    );
    for (target, expected) in [
        ("first", "The first value is selected."),
        ("second", "The second value is selected."),
    ] {
        let (status, answer) = call(&host, "get", get_call(target, "default")).await;
        assert_eq!(status, 200, "{answer}");
        assert_eq!(answer["node"]["statement"], expected);
        assert_eq!(answer["operation"], "get");
        assert!(answer["stamp"]["serial"].is_number());
    }
    first.edit("default", "The saved edit is selected.");
    let (_, answer) = call(&host, "get", get_call("first", "default")).await;
    assert_eq!(answer["node"]["statement"], "The saved edit is selected.");
    let (_, answer) = call(&host, "get", get_call("second", "default")).await;
    assert_eq!(answer["node"]["statement"], "The second value is selected.");
    host.shutdown().await;
}

#[tokio::test]
async fn graph_operations_keep_bounds_stamps_and_opaque_info() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&[("selected", &repo)], &["selected"]);
    for (operation, request) in [
        ("info", json!({})),
        ("search", json!({"text":"shared", "limit":1})),
        ("neighbors", json!({"id":"rule_shared"})),
        ("trace", json!({"id":"rule_shared"})),
    ] {
        let context = if operation == "info" {
            json!({"repository":"selected"})
        } else {
            json!({"repository":"selected","scope":"default"})
        };
        let (status, answer) = call(
            &host,
            operation,
            json!({"context":context,"request":request}),
        )
        .await;
        assert_eq!(status, 200, "{operation}: {answer}");
        assert!(!answer
            .to_string()
            .contains(repo.dir.path().to_str().unwrap()));
        if operation == "info" {
            assert_eq!(answer["repository"], "selected");
            assert!(answer.get("stamp").is_none());
        } else {
            assert_eq!(answer["operation"], operation);
            assert!(answer["stamp"]["instance_id"].is_string());
        }
    }
    for bound in [0, 201] {
        let (status, answer) = call(&host, "search", json!({"context":{"repository":"selected","scope":"default"},"request":{"text":"shared","limit":bound}})).await;
        assert_eq!(status, 400);
        assert_eq!(answer["error"]["kind"], "invalid_input");
        assert_eq!(answer["error"]["field"], "request.limit");
    }
    host.shutdown().await;
}

#[tokio::test]
async fn safe_stale_refusal_preserves_the_checked_revision_and_unit_digests() {
    let repo = Repository::new("The first graph is readable.");
    let host = host(&[("selected", &repo)], &["selected"]);
    let (_, before) = call(&host, "get", get_call("selected", "default")).await;
    repo.edit("default", "The edited graph is readable.");
    let mut request = get_call("selected", "default");
    request["context"]["freshness"] = json!("refuse_stale");
    let (status, refusal) = call(&host, "get", request).await;
    assert_eq!(status, 409, "{refusal}");
    assert_eq!(refusal["error"]["kind"], "stale");
    for field in ["serial", "digest", "instance_id"] {
        assert_eq!(refusal["error"][field], before["stamp"][field]);
    }
    assert!(refusal["error"]["moved"]
        .as_array()
        .unwrap()
        .iter()
        .all(|unit| unit["stored"] != unit["live"]));
    assert!(!refusal
        .to_string()
        .contains(repo.dir.path().to_str().unwrap()));
    host.shutdown().await;
}
