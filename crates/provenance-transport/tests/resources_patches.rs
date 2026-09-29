#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}

use axum::{body::Body, http::Request};
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, host};
use tower::ServiceExt as _;

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
        .header("idempotency-key", "request_transport_edit")
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
