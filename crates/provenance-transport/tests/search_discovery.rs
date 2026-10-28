#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use axum::{body::Body, http::Request};
use provenance_macros::verifies;
use serde_json::Value;
use support::records::Repository;
use support::resource_http::{call, host};
use tower::ServiceExt as _;

#[tokio::test]
#[verifies("rule_porcelain_search_text_optional", examples)]
#[verifies("rule_porcelain_search_predicates_intersect", examples)]
async fn public_collection_search_accepts_kind_only_and_combined_queries() {
    let repo = Repository::new("The shared graph is searchable.");
    let host = host(&repo, false);

    let (status, kind_only) = call(&host, "GET", "/rules?query=search", None).await;
    assert_eq!(status, 200, "{kind_only}");
    assert_eq!(kind_only["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(kind_only["data"]["items"][0]["node_type"], "rule");
    assert!(!kind_only["meta"]["has_more"].as_bool().unwrap());

    let (status, live) = call(
        &host,
        "GET",
        "/rules?query=search&text=shared&exclude_terminal=true",
        None,
    )
    .await;
    assert_eq!(status, 200, "{live}");
    assert_eq!(live["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(live["data"]["items"][0]["id"], "rule_shared");

    let (status, combined) = call(&host, "GET", "/rules?query=search&text=absent", None).await;
    assert_eq!(status, 200, "{combined}");
    assert_eq!(
        combined["data"]["items"].as_array().unwrap().as_slice(),
        [] as [serde_json::Value; 0]
    );
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

#[tokio::test]
async fn resolve_symbol_returns_matched_sites_over_http() {
    let repo = Repository::new("The shared graph is readable.");
    std::fs::write(
        repo.dir.path().join("code.rs"),
        concat!(
            "#[rule(\"rule_shared\")]\n",
            "fn pay() {}\n",
            "#[verifies(\"rule_shared\", examples)]\n",
            "fn checks_pay() {}\n",
        ),
    )
    .unwrap();
    let host = host(&repo, false);

    let path = "/rules?query=resolve-symbol&file=code.rs&symbol=pay";
    let (status, body) = call(&host, "GET", path, None).await;

    assert_eq!(status, 200, "{body}");
    let matches = body["data"]["matches"].as_array().unwrap();
    assert!(matches
        .iter()
        .any(|site| site["match_kind"] == "symbol" && site["item_name"] == "pay"));
    assert!(matches
        .iter()
        .any(|site| site["match_kind"] == "file" && site["item_name"] == "checks_pay"));
}
