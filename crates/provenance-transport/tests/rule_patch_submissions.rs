//! A PATCH to a Rule, with or without If-Match, opens one review submission
//! for each new content, and none for identical content or for content that
//! a reviewer accepted. A PATCH of the status to active or archived opens a
//! status request.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_macros::verifies;
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, call_with_headers, host};

const RULE: &str = "/rules/rule_a";

/// The number of review submissions of `rule_a` in the Proposal list.
async fn submissions(host: &StatementHost) -> usize {
    let (status, listed) = call(host, "GET", "/proposals", None).await;
    assert_eq!(status, 200, "{listed}");
    listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|proposal| proposal.to_string().contains("review:rule:rule_a:"))
        .count()
}

/// The status that the pending submission of `rule_a` requests.
async fn requested_status(host: &StatementHost) -> Value {
    let rule = read(host).await;
    let proposal = rule["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no pending submission: {rule}"))
        .to_owned();
    let (status, card) = call(host, "GET", &format!("/proposals/{proposal}"), None).await;
    assert_eq!(status, 200, "{card}");
    card["data"]["record_revision"]["status_request"]["status"].clone()
}

async fn read(host: &StatementHost) -> Value {
    let (status, read) = call(host, "GET", RULE, None).await;
    assert_eq!(status, 200, "{read}");
    read["data"].clone()
}

/// Changes the Rule statement, with the current edit etag when `guarded`.
async fn patch(host: &StatementHost, statement: &str, guarded: bool) {
    let body = Some(json!({"data":{"statement":statement}}));
    let (status, changed) = if guarded {
        let etag = read(host).await["edit"]["etag"]
            .as_str()
            .unwrap()
            .to_owned();
        call_with_headers(host, "PATCH", RULE, body, &[("if-match", &etag)]).await
    } else {
        call(host, "PATCH", RULE, body).await
    };
    assert_eq!(status, 200, "{changed}");
}

/// One pending submission is bound to the current revision.
async fn assert_pending_at_current(host: &StatementHost) {
    let rule = read(host).await;
    let pending = &rule["decision"]["pending"];
    assert!(!pending.is_null(), "no pending submission: {rule}");
    assert_eq!(pending["revision"], rule["edit"]["revision"], "{rule}");
}

#[tokio::test]
#[verifies("rule_content_change_opens_review_submission", examples)]
async fn rule_patches_open_one_submission_for_each_new_content() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo, true);
    let (status, created) = call(
        &host,
        "POST",
        "/rules",
        Some(json!({"data":{
            "id":"rule_a", "statement":"The host keeps each record.", "status":"active",
            "severity":"high", "requirement_ids":["req_shared"], "resolution_ids":[]
        }})),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    assert_eq!(submissions(&host).await, 1);
    // A new active Rule has the review status, and its creation submission
    // requests the active status.
    let created = read(&host).await;
    assert_eq!(created["status"], "review", "{created}");
    assert_eq!(requested_status(&host).await, "active");
    let proposal = created["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, decided) = call(
        &host,
        "POST",
        &format!("{RULE}/submissions/{proposal}/decide"),
        Some(json!({"data":{
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"rule","artifact_id":"rule_a"},
            "feedback":null, "declared_by":null
        }})),
    )
    .await;
    assert_eq!(status, 200, "{decided}");
    assert_eq!(read(&host).await["status"], "active");

    for (guarded, count) in [(false, 2), (true, 3)] {
        let statement = format!("The host keeps each record for {count} days.");
        patch(&host, &statement, guarded).await;
        assert_eq!(submissions(&host).await, count, "guarded: {guarded}");
        assert_pending_at_current(&host).await;
        // The same content again adds nothing.
        patch(&host, &statement, guarded).await;
        assert_eq!(submissions(&host).await, count, "guarded: {guarded}");
    }

    // The accepted content returns: no submission, and the acceptance holds.
    patch(&host, "The host keeps each record.", true).await;
    assert_eq!(submissions(&host).await, 3);
    let rule = read(&host).await;
    assert!(rule["decision"]["pending"].is_null(), "{rule}");
    assert!(!rule["decision"]["current_acceptance"].is_null(), "{rule}");
    assert_eq!(rule["status"], "active");
}

#[tokio::test]
#[verifies("rule_status_change_waits_for_review", examples)]
#[verifies("rule_status_request_changes_one_field", examples)]
async fn a_status_patch_opens_a_status_request_and_keeps_the_status() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo, true);
    let (status, created) = call(
        &host,
        "POST",
        "/rules",
        Some(json!({"data":{
            "id":"rule_a", "statement":"The host keeps each record.", "status":"draft",
            "severity":"high", "requirement_ids":["req_shared"], "resolution_ids":[]
        }})),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let proposal = read(&host).await["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, decided) = call(
        &host,
        "POST",
        &format!("{RULE}/submissions/{proposal}/decide"),
        Some(json!({"data":{
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"rule","artifact_id":"rule_a"},
            "feedback":null, "declared_by":null
        }})),
    )
    .await;
    assert_eq!(status, 200, "{decided}");

    let (status, mixed) = call(
        &host,
        "PATCH",
        RULE,
        Some(json!({"data":{"status":"active","severity":"low"}})),
    )
    .await;
    assert_ne!(status, 200, "{mixed}");
    assert!(read(&host).await["decision"]["pending"].is_null());

    let (status, changed) = call(&host, "PATCH", RULE, Some(json!({"data":{"status":"active"}}))).await;
    assert_eq!(status, 200, "{changed}");
    assert_eq!(changed["data"]["status"], "draft", "{changed}");
    let rule = read(&host).await;
    assert_eq!(rule["status"], "draft", "{rule}");
    assert_eq!(rule["severity"], "high", "{rule}");
    assert_eq!(requested_status(&host).await, "active");
}
