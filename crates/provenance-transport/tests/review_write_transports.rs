//! One review decision gives one result through HTTP, MCP and the native Store.
#![cfg(feature = "test-fixture")]
mod support {
    #[allow(dead_code)]
    pub mod api_fixture;
    pub mod records;
    #[allow(dead_code)]
    pub mod resource_http;
}
use provenance_core::{NodeType, StableId};
use provenance_macros::verifies;
use provenance_store::{review::DecideRecordReview, state_store::StateStore};
use serde_json::{json, Value};
use support::api_fixture::{write_host, ApiSession};
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, call_with_headers};

/// A rejection with feedback, as the review page sends it.
fn decision() -> Value {
    json!({
        "actor":{"identity_type":"human","id":"reviewer"},
        "decision":"rejected", "rationale":"Name the scope.",
        "canonical_artifact":null,
        "feedback":{"role":"user","body":"Name the scope."},
        "declared_by":null
    })
}

/// A repository whose Requirement has one pending review submission.
async fn pending() -> (Repository, String) {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = write_host(&repo);
    let (status, read) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    let etag = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let (status, saved) = call_with_headers(
        &host,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"agent","description":"Enrolled."}})),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    let proposal = saved["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    (repo, proposal)
}

/// Replaces the server-created ids of one repository with fixed names.
fn masked(value: &Value, ids: &[(&str, &str)]) -> Value {
    match value {
        Value::String(text) => ids
            .iter()
            .find(|(id, _)| id == text)
            .map_or_else(|| value.clone(), |(_, name)| json!(name)),
        Value::Array(items) => items.iter().map(|item| masked(item, ids)).collect(),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, field)| (key.clone(), masked(field, ids)))
            .collect(),
        other => other.clone(),
    }
}

/// The decision receipt and the record's review state after the decision,
/// with server-created ids replaced.
async fn observed(repo: &Repository, proposal: &str, receipt: &Value) -> (Value, Value) {
    let ids = [
        (proposal, "proposal"),
        (receipt["id"].as_str().unwrap(), "entry"),
        (receipt["disposition_id"].as_str().unwrap(), "disposition"),
        (receipt["feedback_message_id"].as_str().unwrap(), "feedback"),
    ];
    let (status, read) = call(&write_host(repo), "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    (
        masked(receipt, &ids),
        masked(&read["data"]["decision"], &ids),
    )
}

#[tokio::test]
#[verifies("rule_native_entry_points_call_shared_operations", examples)]
async fn one_review_decision_has_one_result_through_http_mcp_and_the_native_store() {
    let path = |proposal: &str| format!("requirements/req_shared/submissions/{proposal}/decide");

    let (http_repo, http_proposal) = pending().await;
    let (status, http) = call(
        &write_host(&http_repo),
        "POST",
        &format!("/{}", path(&http_proposal)),
        Some(json!({ "data": decision() })),
    )
    .await;
    assert_eq!(status, 200, "{http}");

    let (mcp_repo, mcp_proposal) = pending().await;
    let session = ApiSession::start(write_host(&mcp_repo)).await;
    let result = session
        .call(json!({"path": path(&mcp_proposal), "method": "post", "body": decision()}))
        .await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let mcp = result.structured_content.unwrap();
    session.shutdown().await;

    let (native_repo, native_proposal) = pending().await;
    let mut input = decision();
    input["scope_id"] = json!("default");
    input["proposal_id"] = json!(native_proposal);
    let input: DecideRecordReview = serde_json::from_value(input).unwrap();
    let native = native_repo
        .open()
        .store()
        .decide_record_review_for(
            NodeType::Requirement,
            &StableId::new("req_shared").unwrap(),
            input,
        )
        .unwrap();
    let native = serde_json::to_value(native).unwrap();

    assert_eq!(http["data"]["fact"], "decided");
    assert_eq!(http["meta"], mcp["meta"]);
    let http = observed(&http_repo, &http_proposal, &http["data"]).await;
    let mcp = observed(&mcp_repo, &mcp_proposal, &mcp["data"]).await;
    let native = observed(&native_repo, &native_proposal, &native).await;
    assert_eq!(mcp, http);
    assert_eq!(native, http);
}
