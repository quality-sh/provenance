#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use axum::{body::Body, http::Request};
use provenance_core::Manifest;
use provenance_macros::verifies;
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

fn allow_reviewer(repo: &Repository) {
    let mut manifest: Manifest =
        serde_json::from_slice(&std::fs::read(repo.layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        repo.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

async fn call(
    host: &StatementHost,
    method: &str,
    path: &str,
    body: Option<Value>,
    etag: Option<&str>,
) -> (u16, Value, Option<String>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret");
    if let Some(etag) = etag {
        request = request.header("if-match", etag);
    }
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let body = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
    let response = host
        .router()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value, etag)
}

async fn set_statement(host: &StatementHost, statement: &str) -> Value {
    let (status, read, etag) = call(host, "GET", "/requirements/req_flow", None, None).await;
    assert_eq!(status, 200, "{read}");
    let (status, saved, _) = call(
        host,
        "PATCH",
        "/requirements/req_flow",
        Some(json!({"data":{"actor":"agent","statement":statement}})),
        etag.as_deref(),
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    saved["data"].clone()
}

/// Flow: create, accept, change the text to B, then return it to A.
#[tokio::test]
#[verifies("rule_approval_accepts_reviewed_version", examples)]
#[verifies("rule_content_change_opens_review_submission", examples)]
async fn accepted_text_returns_as_accepted() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo);
    let (status, created, _) = call(
        &host,
        "POST",
        "/requirements",
        Some(json!({"data":{
            "actor":"agent","id":"req_flow","statement":"The flow text is A.",
            "status":"active","depends_on":[],"supersedes":[]
        }})),
        None,
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let revision_a = created["data"]["edit"]["revision"].clone();
    let proposal = created["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .expect("creation opens a review submission")
        .to_owned();
    let (status, decided, _) = call(
        &host,
        "POST",
        &format!("/requirements/req_flow/submissions/{proposal}/decide"),
        Some(json!({"data":{
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_flow"},
            "feedback":null, "declared_by":null
        }})),
        None,
    )
    .await;
    assert_eq!(status, 200, "{decided}");
    let disposition = decided["data"]["disposition_id"].clone();

    let changed = set_statement(&host, "The flow text is B.").await;
    assert_ne!(changed["edit"]["revision"], revision_a);
    assert!(changed["decision"]["current_acceptance"].is_null());
    assert!(changed["decision"]["pending"]["proposal_id"].is_string());

    let returned = set_statement(&host, "The flow text is A.").await;
    assert_eq!(returned["edit"]["revision"], revision_a);
    assert!(returned["decision"]["pending"].is_null(), "{returned}");
    let acceptance = &returned["decision"]["current_acceptance"];
    assert_eq!(acceptance["revision"], revision_a);
    assert_eq!(acceptance["disposition"]["id"], disposition);
}
