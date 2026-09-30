#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use axum::{body::Body, http::Request};
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
    .unwrap()
    .allow_writes();
    StatementHost::with_fixture_access(access)
}

async fn call(
    host: &StatementHost,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret");
    let has_body = body.is_some();
    let body = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
    if has_body {
        request = request.header("content-type", "application/json");
    }
    let response = host
        .router()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value)
}

#[tokio::test]
async fn addressed_discussion_member_reads_cover_all_six_parent_kinds() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);
    for (plural, id) in [
        ("sources", "source_shared"),
        ("requirements", "req_shared"),
        ("resolutions", "resolution_shared"),
        ("rules", "rule_shared"),
        ("topics", "topic_shared"),
        ("questions", "question_shared"),
    ] {
        let parent = format!("/{plural}/{id}/discussions");
        let (status, started) = call(
            &host,
            "POST",
            &parent,
            Some(json!({"data":{
                "actor":"reviewer", "declared_by":null, "role":"user",
                "body":format!("Discussion for {plural}.")
            }})),
        )
        .await;
        assert_eq!(status, 200, "{plural}: {started}");
        let discussion_id = started["data"]["discussion_id"].as_str().unwrap();
        let (status, read) = call(
            &host,
            "GET",
            &format!("{parent}/{discussion_id}"),
            None,
        )
        .await;
        assert_eq!(status, 200, "{plural}: {read}");
        assert_eq!(read["data"]["discussion"]["discussion_id"], discussion_id);
    }
}

#[tokio::test]
async fn repeated_discussion_start_after_a_lost_response_creates_another_discussion() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);
    let body = json!({"data":{
        "actor":"reviewer", "declared_by":null, "role":"user",
        "body":"A response can be lost."
    }});

    let (_, first) = call(
        &host,
        "POST",
        "/requirements/req_shared/discussions",
        Some(body.clone()),
    )
    .await;
    let (status, second) = call(
        &host,
        "POST",
        "/requirements/req_shared/discussions",
        Some(body),
    )
    .await;

    assert_eq!(status, 200, "{second}");
    assert_ne!(first["data"]["discussion_id"], second["data"]["discussion_id"]);
    assert_ne!(first["data"]["request_id"], second["data"]["request_id"]);
}

#[tokio::test]
async fn repeated_requirement_create_returns_a_typed_conflict() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo);
    let body = json!({"data":{
        "actor":"reviewer", "id":"req_created", "statement":"One statement.",
        "status":"discovery", "depends_on":[], "supersedes":[]
    }});

    let (status, first) = call(&host, "POST", "/requirements", Some(body.clone())).await;
    assert_eq!(status, 200, "{first}");
    let (status, repeated) = call(&host, "POST", "/requirements", Some(body)).await;

    assert_eq!(status, 409, "{repeated}");
    assert_eq!(repeated["error"]["kind"], "already_exists");
}
