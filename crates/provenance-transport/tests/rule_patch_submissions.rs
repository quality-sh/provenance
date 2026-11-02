//! A PATCH to a Rule, with or without If-Match, opens one review submission
//! for each new content, and none for identical content or for content that
//! a reviewer accepted.
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
