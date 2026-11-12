//! Answers over an id on two lines that this change keeps. A write that does
//! not run the aggregate check of the ideation records succeeds over a
//! proposal id on two lines: a withdrawal of a review submission and a
//! typed-spec apply that opens no review submission. The read of a
//! Requirement over a disposition id on two lines gives its decision two
//! times.
//!
//! No Rule: the tests pin answers that this change keeps.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod ideation;
    pub mod ideation_ids;
    pub mod records;
    pub mod resource_http;
}

use serde_json::{json, Value};
use support::ideation_ids::{at_stored, copy_row, ideation_repository, read_answers, state_path};
use support::resource_http::{call, call_with_headers, host};

#[tokio::test]
async fn a_withdrawal_over_a_different_proposal_id_on_two_lines_is_written() {
    let (repo, _) = ideation_repository().await;
    let writer = host(&repo, true);
    let (status, read) = call(&writer, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    let etag = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let (status, saved) = call_with_headers(
        &writer,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"agent", "description":"Enrolled."}})),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    let submission = saved["data"]["decision"]["pending"]["proposal_id"].clone();
    copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    let cards = state_path(&repo, "proposal_cards.jsonl");
    let stored = std::fs::read_to_string(&cards).unwrap();
    let path = format!(
        "/requirements/req_shared/submissions/{}/withdraw",
        submission.as_str().unwrap()
    );
    let body = json!({"data":{"actor":"agent", "declared_by":null, "reason":"Revise it."}});
    let (status, answer) = call(&writer, "POST", &path, Some(body)).await;
    let withdrawals = std::fs::read_to_string(state_path(&repo, "withdrawals.jsonl")).unwrap();
    let withdrawal: Value = serde_json::from_str(withdrawals.lines().last().unwrap()).unwrap();
    let expected = json!({"data":{"scope_id":"default", "id":withdrawal["id"],
        "requirement_id":"req_shared", "proposal_id":submission, "sequence":2,
        "fact":"withdrawn", "actor":"agent"}, "meta":{}});
    assert_eq!((status, answer), (200, expected));
    assert_eq!(std::fs::read_to_string(&cards).unwrap(), stored);
}

#[tokio::test]
async fn a_typed_apply_that_opens_no_submission_over_a_proposal_id_on_two_lines_is_written() {
    let (repo, _) = ideation_repository().await;
    copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    let writer = host(&repo, true);
    let spec = json!({"data":{"spec":"fx", "declared_by":"spec://fx",
        "sources":[{"key":"src", "name":"Original", "kind":"policy"}], "requirements":[]}});
    let (status, answer) = call(&writer, "POST", "/authoring-changes", Some(spec)).await;
    let source = answer["data"]["resources"][0]["id"].clone();
    let expected = json!({"data":{"declared_by":"spec://fx", "created":1, "updated":0,
        "moved":0, "deleted":0, "conflicts":0, "unchanged":0,
        "resources":[{"kind":"source", "key":"src", "address":["fx", "source", "src"],
            "id":source, "state":"created"}]}, "meta":{}});
    assert_eq!((status, answer), (200, expected));
}

/// No Rule: it pins an answer that this change keeps, and the decision two
/// times is a fault of its own.
#[tokio::test]
async fn a_requirement_read_over_a_disposition_id_on_two_lines_gives_the_decision_two_times() {
    let (repo, healthy) = ideation_repository().await;
    let (first, second) = copy_row(&repo, "dispositions.jsonl", "disposition_a");
    let fault = format!(
        "scopes/default/ideation/dispositions.jsonl line {second}: duplicate immutable \
        disposition id disposition_a at line {first} and at line {second}"
    );
    let mut expected = at_stored(&healthy[..1], &fault);
    let decisions = &mut expected[0]["body"]["data"]["decision"]["decisions"];
    let decision = decisions[0].clone();
    decisions.as_array_mut().unwrap().push(decision);
    assert_eq!(read_answers(&repo).await[..1], expected);
}
