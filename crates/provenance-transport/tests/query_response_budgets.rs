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

fn add_child(repo: &Repository, index: usize, description_bytes: usize) {
    let store = StateStore::new(repo.layout.clone());
    let scope = ScopeId::new("default").unwrap();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope,
            id: StableId::new(format!("req_large_{index:03}")).unwrap(),
            statement: "The graph includes this record.".into(),
            description: Some("x".repeat(description_bytes)),
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

fn add_large_children(repo: &Repository, count: usize) {
    for index in 0..count {
        add_child(repo, index, 60_000);
    }
}

fn set_requirement_description(repo: &Repository, id: &str, description_bytes: usize) {
    let scope = ScopeId::new("default").unwrap();
    let path = provenance_store::shards::requirements_path(&repo.layout, &scope);
    let records = std::fs::read_to_string(&path).unwrap();
    let rewritten = records
        .lines()
        .map(|line| {
            let mut record: Value = serde_json::from_str(line).unwrap();
            if record["id"].as_str() == Some(id) {
                record["description"] = json!("x".repeat(description_bytes));
            }
            serde_json::to_string(&record).unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{rewritten}\n")).unwrap();
}

async fn public_query_at_boundary(extra_bytes: usize) -> (u16, Vec<u8>, Value) {
    let repo = Repository::new("The shared rule is readable.");
    add_large_children(&repo, 18);
    add_child(&repo, 18, 0);
    let host = host(&repo);
    let path = "/requirements/req_shared?query=neighbors&direction=both&limit=200";
    let (status, baseline, value) = call(&host, path).await;
    assert_eq!(status, 200, "{value}");
    let remaining = QUERY_RESPONSE_BYTES.checked_sub(baseline.len()).unwrap();
    assert!(remaining + extra_bytes < 60_000);

    set_requirement_description(&repo, "req_large_018", remaining + extra_bytes);
    call(&host, path).await
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

#[tokio::test]
async fn public_query_wire_accepts_the_exact_finalized_limit() {
    let (status, bytes, value) = public_query_at_boundary(0).await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(bytes.len(), QUERY_RESPONSE_BYTES);
}

#[tokio::test]
async fn public_query_wire_refuses_one_byte_over_the_finalized_limit() {
    let (status, bytes, value) = public_query_at_boundary(1).await;
    assert_eq!(status, 409, "{value}");
    assert_eq!(value["error"], json!({"kind": "page_budget_exceeded"}));
    assert!(bytes.len() <= QUERY_RESPONSE_BYTES);
}

/// The query response cap belongs to query routes alone: the same store
/// where the query route refuses serves its ordinary member read, whose
/// envelope carries no query limit at all.
#[tokio::test]
async fn ordinary_member_wire_does_not_inherit_the_query_refusal() {
    let repo = Repository::new("The shared rule is readable.");
    add_large_children(&repo, 20);
    let host = host(&repo);

    let query_path = "/requirements/req_shared?query=neighbors&direction=both&limit=200";
    let (status, _, value) = call(&host, query_path).await;
    assert_eq!(status, 409, "{value}");
    assert_eq!(value["error"], json!({"kind": "page_budget_exceeded"}));

    let (status, _, value) = call(&host, "/requirements/req_large_000").await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["id"], "req_large_000");
}
