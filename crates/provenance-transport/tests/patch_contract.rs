#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}

use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, host};

async fn read(host: &provenance_transport::StatementHost, path: &str) -> Value {
    let (status, value) = call(host, "GET", path, None).await;
    assert_eq!(status, 200, "{value}");
    value["data"].clone()
}

async fn create_contribution(host: &provenance_transport::StatementHost) {
    let data = json!({
        "id":"contribution_patch_contract",
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "participant_slot":"reviewer", "stance":"support",
        "strongest_finding":"Original finding", "evidence_references":[],
        "material_claims":[], "risks":[], "objections":[], "challenges":[],
        "suggested_artifact_changes":[], "unsupported_recommendations":[],
        "uncertainty":{"level":"low","rationale":"Direct evidence"},
        "open_questions":[]
    });
    let (status, value) = call(host, "POST", "/contributions", Some(json!({"data": data}))).await;
    assert_eq!(status, 200, "{value}");
}

#[tokio::test]
async fn patch_omission_preserves_and_registered_null_clears() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let path = "/sources/source_shared";

    let (status, value) = call(
        &host,
        "PATCH",
        path,
        Some(json!({"data":{"url":"https://example.test/source"}})),
    )
    .await;
    assert_eq!(status, 200, "{value}");

    let (status, value) = call(
        &host,
        "PATCH",
        path,
        Some(json!({"data":{"name":"Renamed source"}})),
    )
    .await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["url"], "https://example.test/source");

    let (status, value) = call(&host, "PATCH", path, Some(json!({"data":{"url":null}}))).await;
    assert_eq!(status, 200, "{value}");
    assert!(value["data"]["url"].is_null());
}

#[tokio::test]
async fn patch_refuses_native_clear_fields_before_mutation() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let path = "/sources/source_shared";
    let before = read(&host, path).await;

    let (status, failure) = call(
        &host,
        "PATCH",
        path,
        Some(json!({"data":{"clear_fields":["url"]}})),
    )
    .await;
    assert_eq!(status, 400, "{failure}");
    assert_eq!(failure["error"]["kind"], "invalid_input");
    assert_eq!(read(&host, path).await, before);
}

#[tokio::test]
async fn patch_refuses_null_for_graph_and_ideation_fields_without_mutation() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    create_contribution(&host).await;

    for (path, field) in [
        ("/sources/source_shared", "name"),
        (
            "/contributions/contribution_patch_contract",
            "strongest_finding",
        ),
    ] {
        let before = read(&host, path).await;
        let (status, failure) =
            call(&host, "PATCH", path, Some(json!({"data": {field: null}}))).await;
        assert_eq!(status, 400, "{path}: {failure}");
        assert_eq!(failure["error"]["kind"], "invalid_input");
        assert_eq!(read(&host, path).await, before, "{path}");
    }
}
