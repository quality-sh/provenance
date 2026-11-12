//! A proposal, assertion or disposition id is immutable. When a hand edit
//! or a branch merge puts one id on two lines of its file, each ideation
//! write refuses with `invalid_state_file` at the second line, and the
//! message names the two lines. A resource read loads the proposals and the
//! dispositions of the review cycle, and refuses in the same way. A list
//! read answers from the projection and gives the same fault as the
//! `freshness_error` of its catch-up.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod ideation_ids;
    pub mod records;
    pub mod resource_http;
}

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::ideation_ids::{answers, copy_row, ideation_repository, state_path, IDEATION};
use support::records::Repository;
use support::resource_http::{call, call_with_headers, host};

/// The refusal of an id on two lines of the file `name`.
fn located(name: &str, kind: &str, id: &str, (first, second): (usize, usize)) -> Value {
    let message =
        format!("duplicate immutable {kind} id {id} at line {first} and at line {second}");
    json!({"kind":"invalid_state_file", "file":format!("{IDEATION}/{name}"), "line":second,
        "message":message})
}

/// The answers when the fault `refusal` is in a file that the resource read
/// of `req_shared` loads (`loaded`) or does not load.
fn expected(refusal: &Value, loaded: bool) -> Vec<Value> {
    let refused = |request: &str| {
        json!({"request":request, "status":500, "error":refusal,
            "freshness_error":null})
    };
    let file = refusal["file"].as_str().unwrap();
    let (line, message) = (&refusal["line"], refusal["message"].as_str().unwrap());
    let stale = |request: &str| {
        json!({"request":request, "status":200, "error":null,
            "freshness_error":format!("{file} line {line}: {message}")})
    };
    let read = if loaded {
        refused("GET /requirements/req_shared")
    } else {
        stale("GET /requirements/req_shared")
    };
    vec![
        refused("POST /proposals"),
        refused("POST /proposals/proposal_b/assertions"),
        refused("POST /proposals/proposal_b/dispositions"),
        read,
        stale("GET /proposals"),
        stale("GET /assertions"),
        stale("GET /dispositions"),
    ]
}

async fn copied(name: &str, kind: &str, id: &str) -> (Repository, Value) {
    let repo = ideation_repository().await;
    let lines = copy_row(&repo, name, id);
    (repo, located(name, kind, id, lines))
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_proposal_id_on_two_lines_is_located_at_the_second_line() {
    let (repo, refusal) = copied("proposal_cards.jsonl", "proposal", "proposal_a").await;
    assert_eq!(answers(&repo).await, expected(&refusal, true));
}

/// The resource read does not load the assertions, so it answers from the
/// projection.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_assertion_id_on_two_lines_is_located_at_the_second_line() {
    let (repo, refusal) = copied("assertions.jsonl", "assertion", "assertion_a").await;
    assert_eq!(answers(&repo).await, expected(&refusal, false));
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_disposition_id_on_two_lines_is_located_at_the_second_line() {
    let (repo, refusal) = copied("dispositions.jsonl", "disposition", "disposition_a").await;
    assert_eq!(answers(&repo).await, expected(&refusal, true));
}

/// The refusal names the first line and the second line, and not the third.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_id_on_three_lines_is_located_at_the_second_line() {
    let repo = ideation_repository().await;
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_a", lines);
    assert_eq!(answers(&repo).await, expected(&refusal, true));
}

/// With an id on two lines in the proposal file and in the assertion file,
/// each answer names the proposal file, except the assertion write: the
/// assertion writer reads its own file first.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_id_on_two_lines_in_two_files_is_located_in_the_proposal_file() {
    let repo = ideation_repository().await;
    let proposals = copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    let assertions = copy_row(&repo, "assertions.jsonl", "assertion_a");
    let mut expected = expected(
        &located("proposal_cards.jsonl", "proposal", "proposal_a", proposals),
        true,
    );
    expected[1]["error"] = located("assertions.jsonl", "assertion", "assertion_a", assertions);
    assert_eq!(answers(&repo).await, expected);
}

/// A review submission (an edit of a Requirement) reads the proposals and
/// refuses before it writes.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_review_submission_over_a_proposal_id_on_two_lines_is_located() {
    let repo = ideation_repository().await;
    let writer = host(&repo, true);
    let (status, read) = call(&writer, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    let etag = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    let path = state_path(&repo, "proposal_cards.jsonl");
    let stored = std::fs::read_to_string(&path).unwrap();
    let (status, answer) = call_with_headers(
        &writer,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"agent", "description":"Enrolled."}})),
        &[("if-match", &etag)],
    )
    .await;
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_a", lines);
    assert_eq!((status, answer), (500, json!({"error":refusal, "meta":{}})));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), stored);
}
