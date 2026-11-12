//! A proposal, assertion or disposition id is immutable. When a hand edit
//! or a branch merge puts one id on two lines of its file, each ideation
//! write refuses with `invalid_state_file` at the second line, and the
//! message names the two lines. Each read answers at the stored serial, and
//! its `freshness_error` gives the same fault.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod ideation;
    pub mod ideation_ids;
    pub mod records;
    pub mod resource_http;
}

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::ideation_ids::{
    at_stored, copy_row, ideation_repository, read_answers, refused, state_path, write_answers,
    IDEATION, WRITES,
};
use support::resource_http::{call, call_with_headers, host};

/// The refusal of an id on two lines of the file `name`.
fn located(name: &str, kind: &str, id: &str, (first, second): (usize, usize)) -> Value {
    let message =
        format!("duplicate immutable {kind} id {id} at line {first} and at line {second}");
    json!({"kind":"invalid_state_file", "file":format!("{IDEATION}/{name}"), "line":second,
        "message":message})
}

/// The answers of the three ideation writes when each refuses with `refusal`.
fn refused_writes(refusal: &Value) -> Vec<Value> {
    WRITES.map(|request| refused(request, refusal)).to_vec()
}

/// The `freshness_error` of a read over the fault `refusal`.
fn freshness(refusal: &Value) -> String {
    let (file, line) = (refusal["file"].as_str().unwrap(), &refusal["line"]);
    format!("{file} line {line}: {}", refusal["message"].as_str().unwrap())
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_proposal_id_on_two_lines_refuses_each_write_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_a", lines);
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_assertion_id_on_two_lines_refuses_each_write_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "assertions.jsonl", "assertion_a");
    let refusal = located("assertions.jsonl", "assertion", "assertion_a", lines);
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_disposition_id_on_two_lines_refuses_each_write_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "dispositions.jsonl", "disposition_a");
    let refusal = located("dispositions.jsonl", "disposition", "disposition_a", lines);
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

/// The refusal names the first line and the second line, and not the third.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_id_on_three_lines_is_located_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_a", lines);
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

/// With an id on two lines in the proposal file and in the assertion file,
/// each write names the proposal file: the aggregate check of a write checks
/// the proposal ids before the assertion ids.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_id_on_two_lines_in_two_files_is_located_in_the_proposal_file() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    copy_row(&repo, "assertions.jsonl", "assertion_a");
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_a", lines);
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

/// A review submission (an edit of a Requirement) writes a proposal and
/// refuses before it writes.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_review_submission_over_a_proposal_id_on_two_lines_is_located() {
    let (repo, _) = ideation_repository().await;
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

/// The reads of a Requirement and of the three ideation lists after a hand
/// edit that puts the id `id` on two lines of the file `name`.
async fn reads_over_a_copied_row(name: &str, kind: &str, id: &str) -> (Vec<Value>, Vec<Value>) {
    let (repo, healthy) = ideation_repository().await;
    let lines = copy_row(&repo, name, id);
    let fault = freshness(&located(name, kind, id, lines));
    (read_answers(&repo).await, at_stored(&healthy, &fault))
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_a_proposal_id_on_two_lines_answers_at_the_stored_serial() {
    let (answers, expected) =
        reads_over_a_copied_row("proposal_cards.jsonl", "proposal", "proposal_a").await;
    assert_eq!(answers, expected);
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_an_assertion_id_on_two_lines_answers_at_the_stored_serial() {
    let (answers, expected) =
        reads_over_a_copied_row("assertions.jsonl", "assertion", "assertion_a").await;
    assert_eq!(answers, expected);
}

/// The read of the Requirement builds its review decisions from the stored
/// rows, so it gives the decision of `disposition_a` two times.
#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_a_disposition_id_on_two_lines_answers_at_the_stored_serial() {
    let (answers, mut expected) =
        reads_over_a_copied_row("dispositions.jsonl", "disposition", "disposition_a").await;
    let decisions = &mut expected[0]["body"]["data"]["decision"]["decisions"];
    let decision = decisions[0].clone();
    decisions.as_array_mut().unwrap().push(decision);
    assert_eq!(answers, expected);
}

/// The catch-up checks the proposal ids before the assertion ids.
#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_ids_on_two_lines_in_two_files_names_the_proposal_file() {
    let (repo, healthy) = ideation_repository().await;
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    copy_row(&repo, "assertions.jsonl", "assertion_a");
    let fault = freshness(&located("proposal_cards.jsonl", "proposal", "proposal_a", lines));
    assert_eq!(read_answers(&repo).await, at_stored(&healthy, &fault));
}
