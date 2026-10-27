#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_macros::verifies;
use serde_json::{json, Value};
use support::{
    records::Repository,
    resource_http::{call, call_with_headers, host},
};

async fn patch(host: &provenance_transport::StatementHost, relations: Value) -> (u16, Value) {
    let (_, before) = call(host, "GET", "/requirements/req_shared", None).await;
    call_with_headers(
        host,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{
            "actor":"author",
            "relationships":relations
        }})),
        &[("If-Match", before["data"]["edit"]["etag"].as_str().unwrap())],
    )
    .await
}

#[tokio::test]
#[verifies("rule_requirement_accepts_multiple_parents", examples)]
#[verifies("rule_supersede_swaps_selected_child_parent", examples)]
async fn http_adds_before_removal_and_a_failed_removal_keeps_both_parents() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    for id in ["req_p", "req_b"] {
        let (status, result) = call(
            &host,
            "POST",
            "/requirements",
            Some(json!({"data":{
                "actor":"author", "id":id, "statement":"The system stores records.",
                "status":"discovery", "depends_on":[], "supersedes":[]
            }})),
        )
        .await;
        assert_eq!(status, 200, "{result}");
    }
    let (status, value) = patch(&host, json!({"refines":{"add":["req_p"]}})).await;
    assert_eq!(status, 200, "{value}");
    let (status, value) = patch(&host, json!({"refines":{"add":["req_b"]}})).await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["refines"], json!(["req_b", "req_p"]));
    let (status, _) = patch(&host, json!({"refines":{"remove":["req_missing"]}})).await;
    assert_eq!(status, 404);
    let (_, value) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(value["data"]["refines"], json!(["req_b", "req_p"]));
    let (status, value) = patch(&host, json!({"refines":{"remove":["req_p"]}})).await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["refines"], json!(["req_b"]));
}
