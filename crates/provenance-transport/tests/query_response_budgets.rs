use axum::{body::Body, http::Request};
use provenance_core::protocol::QUERY_RESPONSE_BYTES;
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use tower::ServiceExt as _;

mod support {
    pub mod records;
    pub mod resource_http;
}
use support::{records::Repository, resource_http::host};

async fn call(host: &StatementHost, path: &str) -> (u16, Vec<u8>, Value) {
    let response = host
        .router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(path)
                .header("host", "fixture.test")
                .header("authorization", "Bearer fixture-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    let value = serde_json::from_slice(&bytes).unwrap();
    (status, bytes, value)
}

fn orphan_rules(repo: &Repository) {
    let path = provenance_store::shards::rules_path(
        &repo.layout,
        &provenance_core::ScopeId::new("default").unwrap(),
    );
    let records = std::fs::read_to_string(&path).unwrap();
    let rewritten = records
        .lines()
        .map(|line| {
            let mut record: Value = serde_json::from_str(line).unwrap();
            record["requirement_ids"] = json!([]);
            serde_json::to_string(&record).unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{rewritten}\n")).unwrap();
}

#[tokio::test]
async fn public_query_wire_includes_final_catch_up_failure_metadata() {
    let repo = Repository::new("The shared rule is searchable.");
    let host = host(&repo, false);
    let path = "/rules?query=search&text=searchable&limit=50";
    let (status, _, healthy) = call(&host, path).await;
    assert_eq!(status, 200, "{healthy}");

    orphan_rules(&repo);
    let (status, bytes, value) = call(&host, path).await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(
        value["meta"]["freshness_error"],
        "catch-up failed; answer uses the stored projection"
    );
    assert_eq!(value["meta"]["freshness_cause"], "catch_up_failed");
    assert_eq!(value["meta"]["stamp"]["policy"], "catch_up_failed");
    assert!(value["data"]["items"].is_array());
    assert!(value.get("protocol_version").is_none());
    assert!(value.get("operation").is_none());
    assert!(bytes.len() <= QUERY_RESPONSE_BYTES);
}
