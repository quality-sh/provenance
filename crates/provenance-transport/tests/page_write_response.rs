#![cfg(feature = "test-fixture")]
#[allow(dead_code)]
mod support {
    pub mod records;
    pub mod resource_http;
}
use axum::{body::Body, http::Request};
use provenance_transport::ReviewPageWrite;
use serde_json::json;
use std::sync::{Arc, Mutex};
use support::{records::Repository, resource_http::host};
use tower::ServiceExt as _;

// Implementation aid: the host's observer fails after the public route commits.
// The browser must receive the write result, and recovery must receive a reset.
#[tokio::test]
async fn a_saved_page_write_succeeds_when_notification_fails() {
    let repo = Repository::new("The graph is readable.");
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let seen = notifications.clone();
    let host = host(&repo, true).with_page_writes(
        repo.layout.clone(),
        Arc::new(move |changes| {
            if !changes.configuration_changed {
                panic!("notification unavailable");
            }
            seen.lock().unwrap().push(changes);
        }),
    );
    let mut request = Request::builder()
        .method("POST")
        .uri("/rules")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"data":{
                "id":"rule_saved", "statement":"The saved write succeeds.", "status":"draft",
                "severity":"high", "requirement_ids":["req_shared"], "resolution_ids":[]
            }})
            .to_string(),
        ))
        .unwrap();
    request.extensions_mut().insert(ReviewPageWrite);
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(std::fs::read_to_string(
        repo.dir
            .path()
            .join(".provenance/state/scopes/default/rules/rule.jsonl")
    )
    .unwrap()
    .contains("rule_saved"));
    assert_eq!(status.as_u16(), 200, "{}", String::from_utf8_lossy(&body));
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["data"]["id"], "rule_saved");
    let notifications = notifications.lock().unwrap();
    assert_eq!(notifications.len(), 1);
    assert!(notifications[0].configuration_changed);
    assert!(notifications[0].scopes.is_empty());
}
