#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use axum::{body::Body, http::Request};
use serde_json::Value;
use support::records::Repository;
use support::resource_http::{call, host};
use tower::ServiceExt as _;

#[tokio::test]
async fn public_collection_search_accepts_kind_only_and_combined_queries() {
    let repo = Repository::new("The shared graph is searchable.");
    let host = host(&repo, false);

    let (status, kind_only) = call(&host, "GET", "/rules?query=search", None).await;
    assert_eq!(status, 200, "{kind_only}");
    assert_eq!(kind_only["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(kind_only["data"]["items"][0]["node_type"], "rule");
    assert_eq!(kind_only["meta"]["has_more"], false);

    let (status, combined) = call(
        &host,
        "GET",
        "/rules?query=search&text=absent",
        None,
    )
    .await;
    assert_eq!(status, 200, "{combined}");
    assert!(combined["data"]["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn public_collection_search_keeps_authentication() {
    let repo = Repository::new("The shared graph is searchable.");
    let response = host(&repo, false)
        .router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/rules?query=search")
                .header("host", "fixture.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 401);
    let failure: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(failure["error"]["kind"], "unauthenticated");
}
