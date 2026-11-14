//! `provenance check` reports each review row that a read refuses, with the
//! file, the line and the message of the read refusal, and passes on review
//! rows that read as one.

use super::{init, provenance};
use crate::review_host_support::bounded_process::{self, Bounded};
use predicates::str::contains;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

const PROPOSALS: &str = ".provenance/state/scopes/default/ideation/proposal_cards.jsonl";
const WITHDRAWALS: &str = ".provenance/state/scopes/default/ideation/withdrawals.jsonl";

/// A repository with `req_a` and its pending review submission.
fn repository() -> (tempfile::TempDir, Value) {
    let directory = tempfile::tempdir().unwrap();
    init(directory.path());
    bounded_process::provenance()
        .args(["req_a", "create", "--type", "requirement", "--repo"])
        .arg(directory.path())
        .args(["--statement", "The system keeps one copy of each record."])
        .bounded_assert()
        .success();
    let submission = lines(directory.path(), PROPOSALS)
        .into_iter()
        .map(|line| serde_json::from_str::<Value>(&line).unwrap())
        .find(|row| row["proposal_type"] == "record_revision")
        .unwrap();
    (directory, submission)
}

fn lines(repo: &Path, file: &str) -> Vec<String> {
    let text = std::fs::read_to_string(repo.join(file)).unwrap_or_default();
    text.lines().map(str::to_owned).collect()
}

fn append(repo: &Path, file: &str, row: &Value) {
    let mut stored = lines(repo, file);
    stored.push(row.to_string());
    std::fs::create_dir_all(repo.join(file).parent().unwrap()).unwrap();
    std::fs::write(repo.join(file), format!("{}\n", stored.join("\n"))).unwrap();
}

fn withdrawal(id: &str, submission: &Value, reason: &str) -> Value {
    json!({"scope_id":"default", "id":id, "proposal_id":submission["id"], "actor":"agent",
        "reason":reason})
}

/// Flow: a hand edit adds a second submission of `req_a` with its review
/// cycle. `check` exits 1 and names the file, the line and the repair.
#[test]
#[verifies("rule_check_reports_review_row_faults", examples)]
fn check_reports_two_submissions_with_one_cycle_at_the_second_line() {
    let (directory, submission) = repository();
    let mut copied = submission.clone();
    copied["id"] = json!("submission_by_hand");
    append(directory.path(), PROPOSALS, &copied);
    let line = lines(directory.path(), PROPOSALS).len();
    provenance(directory.path())
        .failure()
        .stderr(contains("review row finding(s):"))
        .stderr(contains(format!(
            "scopes/default/ideation/proposal_cards.jsonl line {line}: requirement req_a has two \
             review submissions with review cycle 1:"
        )))
        .stderr(contains("To repair, remove the line of one submission"));
}

/// Flow: a hand edit adds a withdrawal that names no review submission and
/// a withdrawal ID on two lines with different contents.
#[test]
#[verifies("rule_check_reports_review_row_faults", examples)]
fn check_reports_each_withdrawal_that_a_read_refuses() {
    let (directory, submission) = repository();
    let missing = json!({"id":"proposal_missing"});
    append(
        directory.path(),
        WITHDRAWALS,
        &withdrawal("withdrawal_lost", &missing, "Lost."),
    );
    append(
        directory.path(),
        WITHDRAWALS,
        &withdrawal("withdrawal_a", &submission, "First."),
    );
    append(
        directory.path(),
        WITHDRAWALS,
        &withdrawal("withdrawal_a", &submission, "Second."),
    );
    provenance(directory.path())
        .failure()
        .stderr(contains(
            "scopes/default/ideation/withdrawals.jsonl line 1: withdrawal withdrawal_lost names \
             no review submission: proposal_missing",
        ))
        .stderr(contains(
            "scopes/default/ideation/withdrawals.jsonl line 3: withdrawal withdrawal_a is on line \
             2 and line 3 of ideation/withdrawals.jsonl with different contents.",
        ));
}

/// Flow: a hand edit copies the line of a submission and of a withdrawal.
/// The rows read as one, and `check` passes.
#[test]
#[verifies("rule_equal_review_row_lines_read_as_one", examples)]
fn check_passes_on_equal_lines_of_one_review_row() {
    let (directory, submission) = repository();
    append(directory.path(), PROPOSALS, &submission);
    let row = withdrawal("withdrawal_a", &submission, "Start again.");
    append(directory.path(), WITHDRAWALS, &row);
    append(directory.path(), WITHDRAWALS, &row);
    provenance(directory.path()).success();
}
