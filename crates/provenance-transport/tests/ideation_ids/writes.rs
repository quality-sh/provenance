//! The refusal of each ideation write over an id on two lines of its file.

use super::repository::{
    copy_row, copy_row_with_key, ideation_repository, merge_assertion_two_times, refused,
    state_path, write_answers, IDEATION, WRITES,
};
use super::support::resource_http::{call, call_with_headers, host};
use provenance_macros::verifies;
use serde_json::{json, Value};

/// The refusal of the id `id` of the kind `kind` on the lines `first` and
/// `second` of the file `name`.
pub fn located(name: &str, kind: &str, id: &str, (first, second): (usize, usize)) -> Value {
    let message =
        format!("duplicate immutable {kind} id {id} at line {first} and at line {second}");
    json!({"kind":"invalid_state_file", "file":format!("{IDEATION}/{name}"), "line":second,
        "message":message})
}

/// The answers of the three ideation writes when each refuses with `refusal`.
fn refused_writes(refusal: &Value) -> Vec<Value> {
    WRITES.map(|request| refused(request, refusal)).to_vec()
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_proposal_id_on_two_lines_refuses_each_write_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_b");
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_b", lines);
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

/// The typed read of a proposal row also takes its id from the key
/// `proposalId`, and the refusal finds the row by the same read.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_proposal_id_on_two_lines_under_two_keys_refuses_each_write_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row_with_key(&repo, "proposal_cards.jsonl", "proposal_b", "proposalId");
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_b", lines);
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_assertion_id_on_two_lines_refuses_each_write_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = merge_assertion_two_times(&repo);
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

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_contribution_id_on_two_lines_refuses_each_write_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "contributions.jsonl", "contribution_evidence");
    let refusal = located(
        "contributions.jsonl",
        "contribution",
        "contribution_evidence",
        lines,
    );
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn a_synthesis_packet_id_on_two_lines_refuses_each_write_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "synthesis_packets.jsonl", "synthesis_evidence");
    let refusal = located(
        "synthesis_packets.jsonl",
        "synthesis packet",
        "synthesis_evidence",
        lines,
    );
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

/// The refusal names the first line and the second line, and not the third.
#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_id_on_three_lines_is_located_at_the_second_line() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_b");
    copy_row(&repo, "proposal_cards.jsonl", "proposal_b");
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_b", lines);
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

/// With an id on two lines in the proposal file and in the assertion file,
/// each write names the proposal file: the aggregate check of a write checks
/// the proposal ids before the assertion ids.
///
/// No Rule: an implementation aid. The Rule asks for a record at fault, and
/// both files hold one, so the Rule does not state which file a write names.
#[tokio::test]
async fn an_id_on_two_lines_in_two_files_is_located_in_the_proposal_file() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_b");
    merge_assertion_two_times(&repo);
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_b", lines);
    assert_eq!(write_answers(&repo).await, refused_writes(&refusal));
}

/// With an id on two lines in the contribution file and in the proposal
/// file, each write names the contribution file: the aggregate check of a
/// write checks the contribution ids first.
///
/// No Rule: an implementation aid. The Rule asks for a record at fault, and
/// both files hold one, so the Rule does not state which file a write names.
#[tokio::test]
async fn a_contribution_id_and_a_proposal_id_on_two_lines_are_located_in_the_contribution_file() {
    let (repo, _) = ideation_repository().await;
    let lines = copy_row(&repo, "contributions.jsonl", "contribution_evidence");
    copy_row(&repo, "proposal_cards.jsonl", "proposal_b");
    let refusal = located(
        "contributions.jsonl",
        "contribution",
        "contribution_evidence",
        lines,
    );
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
    let lines = copy_row(&repo, "proposal_cards.jsonl", "proposal_b");
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
    let refusal = located("proposal_cards.jsonl", "proposal", "proposal_b", lines);
    assert_eq!((status, answer), (500, json!({"error":refusal, "meta":{}})));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), stored);
}
