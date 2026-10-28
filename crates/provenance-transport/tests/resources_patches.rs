#![cfg(feature = "test-fixture")]
mod support {
    pub mod mcp;
    pub mod records;
    pub mod resource_http;
}

use axum::{body::Body, http::Request};
use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, host};
use tower::ServiceExt as _;

fn source_etag(repo: &Repository) -> String {
    provenance_store::state_store::StateStore::new(repo.layout.clone())
        .record_edit_state(
            &provenance_core::ScopeId::new("default").unwrap(),
            provenance_core::NodeType::Source,
            &provenance_core::StableId::new("source_shared").unwrap(),
        )
        .unwrap()
        .etag
}

#[tokio::test]
async fn draft_patch_refuses_missing_resources() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let contribution = json!({"data":{
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "participant_slot":"reviewer", "stance":"support",
        "strongest_finding":"Observed.", "evidence_references":[], "material_claims":[],
        "risks":[], "objections":[], "challenges":[], "suggested_artifact_changes":[],
        "unsupported_recommendations":[],
        "uncertainty":{"level":"low","rationale":"Direct evidence."}, "open_questions":[]
    }});
    let synthesis = json!({"data":{
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "summary":"No record exists.", "consensus":[], "contested_claims":[],
        "minority_objections":[], "evidence_gaps":[], "unsupported_speculation":[],
        "open_questions":[], "suggested_artifacts":[], "required_human_decisions":[]
    }});
    for (path, body) in [
        ("/contributions/contribution_missing", contribution),
        ("/synthesis-packets/synthesis_missing", synthesis),
    ] {
        let (status, failure) = call(&host, "PATCH", path, Some(body)).await;
        assert_eq!(status, 404, "{failure}");
        assert_eq!(failure["error"]["kind"], "resource_not_found");
        let (status, read) = call(&host, "GET", path, None).await;
        assert_eq!(status, 404, "{read}");
    }
}

#[tokio::test]
async fn requirement_patch_binds_receipt_and_precondition_headers() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let request = Request::builder()
        .method("GET")
        .uri("/requirements/req_shared")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .body(Body::empty())
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let etag = response
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let request = Request::builder()
        .method("PATCH")
        .uri("/requirements/req_shared")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .header("if-match", etag)
        .body(Body::from(
            json!({"data":{"actor":"ben","description":"Edited through the resource route."}})
                .to_string(),
        ))
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let value: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(status, 200, "{value}");
    assert_eq!(
        value["data"]["description"],
        "Edited through the resource route."
    );
    assert!(value["data"]["edit"]["etag"].is_string());
    assert!(value["data"]["decision"].is_object());
}

#[tokio::test]
#[verifies("rule_review_update_guard_header", examples)]
async fn source_patch_refuses_the_second_client_with_a_stale_etag() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let etag = source_etag(&repo);
    let patch = |name: &str| Some(json!({"data":{"name":name}}));

    let (first_status, first) = support::resource_http::call_with_headers(
        &host,
        "PATCH",
        "/sources/source_shared",
        patch("Source B"),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(first_status, 200, "{first}");

    let (second_status, second) = support::resource_http::call_with_headers(
        &host,
        "PATCH",
        "/sources/source_shared",
        patch("Source C"),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(second_status, 409, "{second}");
    assert_eq!(second["error"]["kind"], "record_edit_conflict", "{second}");
    let (read_status, source) = call(&host, "GET", "/sources/source_shared", None).await;
    assert_eq!(read_status, 200, "{source}");
    assert_eq!(source["data"]["name"], "Source B");
}

#[tokio::test]
#[verifies("rule_review_update_guard_header", examples)]
async fn source_patch_refuses_the_guard_in_the_body() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let etag = source_etag(&repo);

    let (status, refused) = call(
        &host,
        "PATCH",
        "/sources/source_shared",
        Some(json!({"data":{"expected_etag":etag,"name":"Source B"}})),
    )
    .await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(refused["error"]["kind"], "invalid_input", "{refused}");
    let (_, source) = call(&host, "GET", "/sources/source_shared", None).await;
    assert_ne!(source["data"]["name"], "Source B");
}

#[tokio::test]
#[verifies("rule_review_update_guard_header", examples)]
async fn mcp_source_update_takes_the_guard_as_if_match() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let etag = source_etag(&repo);
    let session = support::mcp::Session::start(host(&repo, true)).await;
    let update =
        |name: &str| json!({"target":"source_shared","if_match":etag,"data":{"name":name}});

    let current = support::mcp::call(&session.client, "update", update("Source B")).await;
    assert_ne!(current.is_error, Some(true), "{current:?}");
    let stale = support::mcp::call(&session.client, "update", update("Source C")).await;
    assert_eq!(stale.is_error, Some(true), "{stale:?}");
    assert!(
        format!("{stale:?}").contains("record_edit_conflict"),
        "{stale:?}"
    );
    session.close().await;
}
