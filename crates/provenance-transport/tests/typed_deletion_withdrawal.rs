//! When typed-spec apply deletes a record, it withdraws the undecided review
//! submissions of that record in the same publication. A record with an
//! accepted or rejected submission is refused, and nothing is published.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_core::{ScopeId, Withdrawal};
use provenance_macros::verifies;
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, host};

fn spec(sources: &Value, requirements: &Value) -> Value {
    json!({"data":{
        "schema_version":2, "spec":"fx", "declared_by":"spec://fx",
        "sources":sources, "requirements":requirements
    }})
}

async fn post(host: &StatementHost, path: &str, body: Value) -> Value {
    let (status, value) = call(host, "POST", path, Some(body)).await;
    assert_eq!(status, 200, "{path}: {value}");
    value
}

/// Applies a typed spec with one Source and one Requirement, then creates a
/// native Resolution and a native Rule of that Requirement. Each native
/// create opens a pending review submission.
async fn typed_requirement_with_native_children(host: &StatementHost) {
    let applied = post(
        host,
        "/authoring-changes",
        spec(
            &json!([{"key":"src", "name":"Original", "kind":"policy"}]),
            &json!([{"key":"drop", "statement":"The host stops."}]),
        ),
    )
    .await;
    let requirement = applied["data"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["key"] == "drop")
        .unwrap()["id"]
        .clone();
    post(
        host,
        "/resolutions",
        json!({"data":{
            "id":"res_drop", "title":"Stop", "requirement_ids":[requirement],
            "position":"The host stops.", "rationale":"The host stops.",
            "status":"proposed", "supersedes":[], "inputs":[]
        }}),
    )
    .await;
    post(
        host,
        "/rules",
        json!({"data":{
            "id":"rule_drop", "statement":"The host stops in one second.",
            "status":"draft", "severity":"high", "requirement_ids":[requirement],
            "resolution_ids":[]
        }}),
    )
    .await;
}

/// The pending submission of one record, read through its resource.
async fn pending(host: &StatementHost, path: &str) -> String {
    let (status, read) = call(host, "GET", path, None).await;
    assert_eq!(status, 200, "{read}");
    read["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no pending submission: {read}"))
        .to_owned()
}

/// The review submissions in the Proposal list.
async fn submissions(host: &StatementHost) -> Vec<Value> {
    let (status, listed) = call(host, "GET", "/proposals", None).await;
    assert_eq!(status, 200, "{listed}");
    listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|proposal| proposal.to_string().contains("review:"))
        .cloned()
        .collect()
}

fn withdrawals(repo: &Repository) -> Vec<Withdrawal> {
    let scope = ScopeId::new("default").unwrap();
    let path = provenance_store::shards::withdrawals_path(&repo.layout, &scope);
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
#[verifies("rule_typed_deletion_withdraws_undecided_submissions", examples)]
async fn typed_deletion_withdraws_each_undecided_submission_once() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    typed_requirement_with_native_children(&host).await;
    let resolution = pending(&host, "/resolutions/res_drop").await;
    let rule = pending(&host, "/rules/rule_drop").await;
    // The Rule submission is already withdrawn before the deletion.
    post(
        &host,
        &format!("/rules/rule_drop/submissions/{rule}/withdraw"),
        json!({"data":{"actor":"agent", "declared_by":null, "reason":null}}),
    )
    .await;
    let before = submissions(&host).await;

    let applied = post(&host, "/authoring-changes", spec(&json!([]), &json!([]))).await;

    assert_eq!(applied["data"]["deleted"], 4, "{applied}");
    let mut cascade = applied["data"]["cascade"].as_array().unwrap().clone();
    cascade.sort_by_key(|change| change["id"].as_str().unwrap().to_owned());
    assert_eq!(
        cascade,
        [
            json!({"id":"res_drop", "kind":"resolution", "state":"deleted"}),
            json!({"id":"rule_drop", "kind":"rule", "state":"deleted"}),
        ]
    );
    for path in ["/resolutions/res_drop", "/rules/rule_drop"] {
        let (status, read) = call(&host, "GET", path, None).await;
        assert_eq!(status, 404, "{path}: {read}");
    }
    // The submissions stay as history, and each has exactly one Withdrawal.
    assert_eq!(submissions(&host).await, before);
    let withdrawn = withdrawals(&repo);
    assert_eq!(withdrawn.len(), 2, "{withdrawn:?}");
    let of = |proposal: &str| {
        withdrawn
            .iter()
            .find(|withdrawal| withdrawal.proposal_id.as_str() == proposal)
            .unwrap_or_else(|| panic!("{proposal} has no Withdrawal"))
            .actor
            .clone()
    };
    assert_eq!(of(&resolution), "spec://fx");
    assert_eq!(of(&rule), "agent");
    let (status, read) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
}

#[tokio::test]
#[verifies("rule_typed_deletion_withdraws_undecided_submissions", examples)]
async fn typed_deletion_of_a_record_with_a_decided_submission_publishes_nothing() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo, true);
    typed_requirement_with_native_children(&host).await;
    let resolution = pending(&host, "/resolutions/res_drop").await;
    let rule = pending(&host, "/rules/rule_drop").await;
    post(
        &host,
        &format!("/resolutions/res_drop/submissions/{resolution}/decide"),
        json!({"data":{
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"rejected", "rationale":"The position is not clear.",
            "canonical_artifact":null, "feedback":null, "declared_by":null
        }}),
    )
    .await;

    let (status, refused) = call(
        &host,
        "POST",
        "/authoring-changes",
        Some(spec(&json!([]), &json!([]))),
    )
    .await;

    assert_eq!(status, 409, "{refused}");
    assert_eq!(
        refused["error"]["kind"], "enrolled_record_deletion_conflict",
        "{refused}"
    );
    assert_eq!(refused["error"]["record_id"], "res_drop", "{refused}");
    // Nothing is published: the records stay, and the undecided Rule
    // submission is still pending.
    let (status, read) = call(&host, "GET", "/resolutions/res_drop", None).await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(pending(&host, "/rules/rule_drop").await, rule);
    assert!(withdrawals(&repo).is_empty());
}
