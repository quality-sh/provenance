#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use axum::{body::Body, http::Request};
use provenance_core::{ScopeId, StableId};
use provenance_store::{
    review::{CreateReviewRequirement, SaveRequirement},
    state_store::StateStore,
};
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use support::records::Repository;
use tower::ServiceExt as _;

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

async fn call(host: &StatementHost, method: &str, path: &str) -> (u16, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .body(Body::empty())
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value)
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn edit(store: &StateStore, request: &str, description: &str) -> SaveRequirement {
    serde_json::from_value(json!({
        "request_id":request,"actor":"ben",
        "expected_etag":store
            .requirement_edit_state(&scope(), &sid("req_hist"))
            .unwrap()
            .etag,
        "update":{"scope_id":"default","id":"req_hist","description":description},
        "relationships":null
    }))
    .unwrap()
}

#[tokio::test]
async fn requirement_history_pages_report_limit_has_more_and_cursor() {
    let repo = Repository::new("The shared graph is readable.");
    let store = StateStore::new(repo.layout.clone());
    let create: CreateReviewRequirement = serde_json::from_value(json!({
        "request_id":"hist_create","actor":"ben","origin":null,
        "create":{"scope_id":"default","id":"req_hist",
            "statement":"History pages carry paging metadata.",
            "status":"discovery","depends_on":[],"supersedes":[]}
    }))
    .unwrap();
    store.create_review_requirement(create).unwrap();
    for version in 1..=4 {
        let save = edit(
            &store,
            &format!("hist_s{version}"),
            &format!("Description {version}."),
        );
        store.save_requirement(save).unwrap();
    }

    let host = host(&repo);
    let base = "/requirements/req_hist/history?limit=2";
    let mut seen: Vec<String> = Vec::new();
    let mut cursor = String::new();
    let mut exhausted = false;
    for page in 0..3 {
        let url = if page == 0 {
            base.to_owned()
        } else {
            format!("{base}&cursor={cursor}")
        };
        let (status, body) = call(&host, "GET", &url).await;
        assert_eq!(status, 200, "page {page}: {body}");
        assert_eq!(body["meta"]["limit"], 2, "page {page} echoes the limit");
        let items = body["data"]["items"].as_array().unwrap();
        seen.extend(items.iter().map(|item| {
            item["request_id"].as_str().unwrap().to_owned()
        }));
        if body["meta"]["has_more"] == json!(true) {
            assert_eq!(items.len(), 2, "only a full page may continue");
            cursor = body["meta"]["next_cursor"].as_str().unwrap().to_owned();
        } else {
            assert!(
                body["meta"]["next_cursor"].is_null(),
                "an exhausted page has no cursor"
            );
            exhausted = true;
        }
    }
    assert!(exhausted, "five entries at limit two need three pages");
    assert_eq!(
        seen,
        ["hist_create", "hist_s1", "hist_s2", "hist_s3", "hist_s4"],
        "concatenated pages must keep every entry once, in order"
    );

    let (status, empty) = call(&host, "GET", "/requirements/req_absent/history?limit=3").await;
    assert_eq!(status, 200, "{empty}");
    assert!(empty["data"]["items"].as_array().unwrap().is_empty());
    assert_eq!(empty["meta"]["limit"], 3);
    assert_eq!(empty["meta"]["has_more"], false);
}
