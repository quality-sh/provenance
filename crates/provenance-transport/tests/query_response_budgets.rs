use axum::{body::Body, http::Request};
use provenance_core::protocol::QUERY_RESPONSE_BYTES;
use provenance_core::{RequirementStatus, ScopeId, StableId};
use provenance_store::state_store::{CreateRequirementInput, StateStore};
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use tower::ServiceExt as _;

mod support {
    pub mod records;
}
use support::records::Repository;

fn host(repo: &Repository) -> StatementHost {
    use provenance_transport::fixture::{FixtureAccess, Target};
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repo.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap();
    StatementHost::with_fixture_access(access)
}

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

fn add_large_children(repo: &Repository, count: usize) {
    let store = StateStore::new(repo.layout.clone());
    let scope = ScopeId::new("default").unwrap();
    for index in 0..count {
        store
            .create_requirement(CreateRequirementInput {
                scope_id: scope.clone(),
                id: StableId::new(format!("req_large_{index:03}")).unwrap(),
                statement: "The graph includes this record.".into(),
                description: Some("x".repeat(60_000)),
                status: RequirementStatus::Active,
                domain_id: None,
                refines: Some(StableId::new("req_shared").unwrap()),
                depends_on: vec![],
                supersedes: vec![],
                spawned_by: None,
                origin_thread: None,
                origin_message: None,
            })
            .unwrap();
    }
}

#[tokio::test]
async fn public_query_wire_keeps_valid_data_and_bounds_a_typed_refusal() {
    let valid = Repository::new("The shared rule is readable.");
    add_large_children(&valid, 1);
    let path = "/requirements/req_shared?query=neighbors&direction=both&limit=200";
    let (status, bytes, value) = call(&host(&valid), path).await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["id"], "req_shared");
    assert!(value["data"]["neighbors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|neighbor| neighbor["node"]["id"] == "req_large_000"));
    assert!(value["meta"]["stamp"].is_object());
    assert!(bytes.len() <= QUERY_RESPONSE_BYTES);

    let oversized = Repository::new("The shared rule is readable.");
    add_large_children(&oversized, 20);
    let (status, bytes, value) = call(&host(&oversized), path).await;
    assert_eq!(status, 409, "{value}");
    assert_eq!(value["error"], json!({"kind": "page_budget_exceeded"}));
    assert_eq!(value["meta"], json!({}));
    assert!(bytes.len() <= QUERY_RESPONSE_BYTES);
}

#[tokio::test]
async fn public_query_wire_includes_final_catch_up_failure_metadata() {
    let repo = Repository::new("The shared rule is searchable.");
    let host = host(&repo);
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
