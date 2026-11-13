//! The reads over an id on two lines of its file: each answers at the stored
//! serial, and its `freshness_error` gives the located fault.

use crate::repository::{
    at_stored, copy_row, copy_row_with_key, freshness_parts, ideation_repository,
    merge_assertion_two_times, read_answers,
};
use crate::support::records::Repository;
use crate::writes::located;
use provenance_macros::verifies;
use serde_json::Value;

/// The `freshness_error` of a read over the fault `refusal`.
fn freshness(refusal: &Value) -> String {
    let (file, line) = (refusal["file"].as_str().unwrap(), &refusal["line"]);
    format!(
        "{file} line {line}: {}",
        refusal["message"].as_str().unwrap()
    )
}

/// The reads of a Requirement and of the three ideation lists after the
/// hand edit `edit`, which puts the id `id` on two lines of the file `name`:
/// the parts that a failed catch-up sets, and the parts that the Rule
/// expects.
async fn reads_over(
    name: &str,
    kind: &str,
    id: &str,
    edit: impl FnOnce(&Repository) -> (usize, usize),
) -> (Vec<Value>, Vec<Value>) {
    let (repo, healthy) = ideation_repository().await;
    let lines = edit(&repo);
    let fault = freshness(&located(name, kind, id, lines));
    let expected = freshness_parts(&at_stored(&healthy, &fault));
    (freshness_parts(&read_answers(&repo).await), expected)
}

/// The reads after a copy of the row with the id `id` of the file `name`.
async fn reads_over_a_copied_row(name: &str, kind: &str, id: &str) -> (Vec<Value>, Vec<Value>) {
    reads_over(name, kind, id, |repo| copy_row(repo, name, id)).await
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_a_proposal_id_on_two_lines_answers_at_the_stored_serial() {
    let (answers, expected) =
        reads_over_a_copied_row("proposal_cards.jsonl", "proposal", "proposal_b").await;
    assert_eq!(answers, expected);
}

/// The typed read of a proposal row also takes its id from the key
/// `proposalId`, and the fault finds the row by the same read.
#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_a_proposal_id_on_two_lines_under_two_keys_answers_at_the_stored_serial() {
    let name = "proposal_cards.jsonl";
    let (answers, expected) = reads_over(name, "proposal", "proposal_b", |repo| {
        copy_row_with_key(repo, name, "proposal_b", "proposalId")
    })
    .await;
    assert_eq!(answers, expected);
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_an_assertion_id_on_two_lines_answers_at_the_stored_serial() {
    let (answers, expected) = reads_over(
        "assertions.jsonl",
        "assertion",
        "assertion_a",
        merge_assertion_two_times,
    )
    .await;
    assert_eq!(answers, expected);
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_a_disposition_id_on_two_lines_answers_at_the_stored_serial() {
    let (answers, expected) =
        reads_over_a_copied_row("dispositions.jsonl", "disposition", "disposition_a").await;
    assert_eq!(answers, expected);
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_a_contribution_id_on_two_lines_answers_at_the_stored_serial() {
    let (answers, expected) = reads_over_a_copied_row(
        "contributions.jsonl",
        "contribution",
        "contribution_evidence",
    )
    .await;
    assert_eq!(answers, expected);
}

#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_a_synthesis_packet_id_on_two_lines_answers_at_the_stored_serial() {
    let (answers, expected) = reads_over_a_copied_row(
        "synthesis_packets.jsonl",
        "synthesis packet",
        "synthesis_evidence",
    )
    .await;
    assert_eq!(answers, expected);
}

/// The catch-up checks the proposal ids before the assertion ids.
#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_ids_on_two_lines_in_two_files_names_the_proposal_file() {
    let (answers, expected) = reads_over("proposal_cards.jsonl", "proposal", "proposal_b", |repo| {
        merge_assertion_two_times(repo);
        copy_row(repo, "proposal_cards.jsonl", "proposal_b")
    })
    .await;
    assert_eq!(answers, expected);
}

/// With an id on two lines in the contribution file and in the proposal
/// file, a read names the proposal file: the catch-up checks the proposal,
/// assertion and disposition ids before the aggregate check, which checks
/// the contribution ids. A write names the contribution file.
#[tokio::test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
async fn a_read_over_a_contribution_id_and_a_proposal_id_on_two_lines_names_the_proposal_file() {
    let (answers, expected) = reads_over("proposal_cards.jsonl", "proposal", "proposal_b", |repo| {
        copy_row(repo, "contributions.jsonl", "contribution_evidence");
        copy_row(repo, "proposal_cards.jsonl", "proposal_b")
    })
    .await;
    assert_eq!(answers, expected);
}
