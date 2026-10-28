#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use axum::{body::Body, http::Request};
use provenance_core::ScopeId;
use provenance_macros::verifies;
use serde_json::{json, Value};
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

fn object_keys(value: &Value) -> Vec<&str> {
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

#[tokio::test]
#[verifies("rule_resolve_symbol_marks_symbol_matches", examples)]
#[verifies("rule_resolve_symbol_ranks_symbol_matches_first", examples)]
#[verifies("rule_resolve_symbol_cursor_continues_the_page", examples)]
async fn resolve_symbol_pages_rank_symbol_matches_first_over_http() {
    let repo = Repository::new("The shared graph is readable.");
    repo.seed(
        ScopeId::new("default").unwrap(),
        "The second graph is readable.",
        "_b",
    );
    std::fs::write(
        repo.dir.path().join("code.rs"),
        concat!(
            "#[rule(\"rule_shared\")]\n",
            "fn audit() {}\n",
            "#[rule(\"rule_shared_b\")]\n",
            "fn pay() {}\n",
            "#[verifies(\"rule_shared\", examples)]\n",
            "fn checks_pay() {}\n",
        ),
    )
    .unwrap();
    let host = host(&repo, false);

    let path = "/rules?query=resolve-symbol&file=code.rs&symbol=pay&limit=1";
    let (status, first) = call(&host, "GET", path, None).await;
    assert_eq!(status, 200, "{first}");
    assert_eq!(
        object_keys(&first["data"]),
        ["file", "items", "matches", "symbol"]
    );
    assert_eq!(first["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(first["data"]["items"][0]["id"], "rule_shared_b");
    assert_eq!(
        first["data"]["matches"],
        json!([{
            "rule_id": "rule_shared_b",
            "role": "implementation",
            "line": 3,
            "item_name": "pay",
            "match_kind": "symbol"
        }])
    );
    assert_eq!(first["meta"]["limit"], 1);
    assert_eq!(first["meta"]["has_more"], true);
    let cursor = first["meta"]["next_cursor"].as_str().unwrap();

    let (status, second) = call(&host, "GET", &format!("{path}&cursor={cursor}"), None).await;
    assert_eq!(status, 200, "{second}");
    assert_eq!(
        object_keys(&second["data"]),
        ["file", "items", "matches", "symbol"]
    );
    assert_eq!(second["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["data"]["items"][0]["id"], "rule_shared");
    assert_eq!(
        second["data"]["matches"],
        json!([
            {
                "rule_id": "rule_shared",
                "role": "implementation",
                "line": 1,
                "item_name": "audit",
                "match_kind": "file"
            },
            {
                "rule_id": "rule_shared",
                "role": "verification",
                "line": 5,
                "item_name": "checks_pay",
                "verification_method": "examples",
                "match_kind": "file"
            }
        ])
    );
    assert_eq!(second["meta"]["has_more"], false);
    assert!(second["meta"].get("next_cursor").is_none());
}

/// Implementation aid: this test has no Rule. It checks that the stale
/// route keeps its commit range and summary in `data` and its page fields
/// in `meta`.
#[tokio::test]
async fn stale_keeps_the_range_and_summary_in_data_over_http() {
    let repo = Repository::new("The shared graph is readable.");
    let base = repo.evidence();
    let host = host(&repo, false);

    let (status, body) = call(
        &host,
        "GET",
        &format!("/rules?query=stale&base={base}"),
        None,
    )
    .await;

    assert_eq!(status, 200, "{body}");
    assert_eq!(
        object_keys(&body["data"]),
        ["base", "files_changed", "head", "items", "summary"]
    );
    assert_eq!(body["data"]["base"], base.as_str());
    let head = body["data"]["head"].as_str().unwrap();
    assert_eq!(head.len(), 40);
    assert_ne!(head, base);
    assert_eq!(body["data"]["files_changed"], 1);
    let summary = &body["data"]["summary"];
    assert_eq!(
        object_keys(summary),
        ["gone", "moved", "total_sites", "touched", "untouched"]
    );
    let sites = body["data"]["items"].as_array().unwrap();
    assert_eq!(summary["total_sites"], sites.len());
    assert!(summary["touched"].as_u64().unwrap() >= 1);
    assert_eq!(body["meta"]["limit"], 50);
    assert_eq!(body["meta"]["has_more"], false);
}
