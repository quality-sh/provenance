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
    let mut copied = submission;
    copied["id"] = json!("submission_by_hand");
    append(directory.path(), PROPOSALS, &copied);
    let line = lines(directory.path(), PROPOSALS).len();
    provenance(directory.path())
        .failure()
        .stderr(contains("review row finding(s):"))
        .stderr(contains(format!(
            "scopes/default/ideation/proposal_cards.jsonl line {line}: requirement req_a has 2 \
             review submissions with review cycle 1. To repair, keep one submission and remove \
             the lines of each other submission, then commit."
        )));
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
             no review submission: proposal_missing. To repair, remove ideation/withdrawals.jsonl \
             line 1, then commit.",
        ))
        .stderr(contains(
            "scopes/default/ideation/withdrawals.jsonl line 3: withdrawal withdrawal_a is on lines \
             2 and 3 of ideation/withdrawals.jsonl with different contents. To repair, keep one \
             of these lines and remove the others, then commit.",
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

const DISPOSITIONS: &str = ".provenance/state/scopes/default/ideation/dispositions.jsonl";

fn decision(id: &str, submission: &Value, result: &str) -> Value {
    let accepted = result == "accepted";
    let mut row = json!({"scope_id":"default", "id":id, "proposal_id":submission["id"],
        "decision":result, "actor":{"identity_type":"human", "id":"reviewer"}});
    if accepted {
        row["canonical_artifact"] = json!({"artifact_type":"requirement", "artifact_id":"req_a"});
    } else {
        row["rationale"] = json!("Name the scope.");
    }
    row
}

/// Removes the lines of `file` at the line numbers `remove`.
fn remove(repo: &Path, file: &str, remove: &[usize]) {
    let kept = lines(repo, file)
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !remove.contains(&(index + 1)))
        .map(|(_, line)| format!("{line}\n"))
        .collect::<String>();
    std::fs::write(repo.join(file), kept).unwrap();
}

/// The read of `req_a` and `provenance check` pass on `repo`.
fn reads_and_check_pass(repo: &Path) {
    bounded_process::provenance()
        .args(["api", "requirements/req_a", "--repo"])
        .arg(repo)
        .bounded_assert()
        .success();
    provenance(repo).success();
}

/// Flow: the manifest names the reviewer. The submission S of `req_a` has an
/// approval and a rejection, and a hand edit copies the line of S and
/// changes its summary. One run of
/// `check` names the two faults. After the repair of each, the read and
/// `check` pass.
#[test]
#[verifies("rule_check_reports_review_row_faults", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn check_reports_each_fault_of_one_record_in_one_run() {
    let (directory, submission) = repository();
    let repo = directory.path();
    bounded_process::provenance()
        .args(["init", "--path", repo.to_str().unwrap(), "--scope", "default"])
        .args(["--path-prefix", ".", "--disposition-actor-id", "reviewer"])
        .bounded_assert()
        .success();
    append(repo, DISPOSITIONS, &decision("decision_a", &submission, "accepted"));
    append(repo, DISPOSITIONS, &decision("decision_b", &submission, "rejected"));
    let mut changed = submission.clone();
    changed["summary"] = json!("Changed by hand.");
    append(repo, PROPOSALS, &changed);
    let copied = lines(repo, PROPOSALS).len();
    let id = submission["id"].as_str().unwrap();
    let first = lines(repo, PROPOSALS)
        .iter()
        .position(|line| line.contains(&format!("\"id\":\"{id}\"")))
        .unwrap()
        + 1;
    provenance(repo)
        .failure()
        .stderr(contains(format!(
            "scopes/default/ideation/proposal_cards.jsonl line {copied}: review submission {id} \
             is on lines {first} and {copied} of ideation/proposal_cards.jsonl with different \
             contents."
        )))
        .stderr(contains(format!(
            "scopes/default/ideation/dispositions.jsonl line 2: review submission {id} of \
             requirement req_a has decisions with different results in \
             ideation/dispositions.jsonl: accepted decision_a at line 1 and rejected decision_b \
             at line 2."
        )));
    remove(repo, PROPOSALS, &[copied]);
    remove(repo, DISPOSITIONS, &[2]);
    reads_and_check_pass(repo);
}

/// Flow: a hand edit adds a withdrawal that names no review submission. The
/// person removes the line that the refusal names.
#[test]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn the_repair_of_a_withdrawal_that_names_no_submission_passes() {
    let (directory, _) = repository();
    let repo = directory.path();
    let missing = json!({"id":"proposal_missing"});
    append(repo, WITHDRAWALS, &withdrawal("withdrawal_lost", &missing, "Lost."));
    provenance(repo).failure().stderr(contains(
        "To repair, remove ideation/withdrawals.jsonl line 1, then commit.",
    ));
    remove(repo, WITHDRAWALS, &[1]);
    reads_and_check_pass(repo);
}

/// Flow: a hand edit adds a withdrawal ID on two lines with different
/// contents. The person keeps one of the lines and removes the other.
#[test]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn the_repair_of_a_withdrawal_id_on_two_lines_passes() {
    let (directory, submission) = repository();
    let repo = directory.path();
    append(repo, WITHDRAWALS, &withdrawal("withdrawal_a", &submission, "First."));
    append(repo, WITHDRAWALS, &withdrawal("withdrawal_a", &submission, "Second."));
    provenance(repo).failure().stderr(contains(
        "withdrawal withdrawal_a is on lines 1 and 2 of ideation/withdrawals.jsonl",
    ));
    remove(repo, WITHDRAWALS, &[2]);
    reads_and_check_pass(repo);
}

/// Flow: a hand edit adds two more submissions of `req_a` with the review
/// cycle of its first submission. The refusal names the three, and the
/// person keeps the first and removes the lines of the two others.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn the_repair_of_three_submissions_with_one_cycle_passes() {
    let (directory, submission) = repository();
    let repo = directory.path();
    for id in ["submission_b", "submission_c"] {
        let mut copied = submission.clone();
        copied["id"] = json!(id);
        append(repo, PROPOSALS, &copied);
    }
    let count = lines(repo, PROPOSALS).len();
    provenance(repo).failure().stderr(contains(format!(
        "requirement req_a has 3 review submissions with review cycle 1. To repair, keep one \
         submission and remove the lines of each other submission, then commit. The lines of \
         {}: ideation/proposal_cards.jsonl line {}. The lines of submission_b: \
         ideation/proposal_cards.jsonl line {}. The lines of submission_c: \
         ideation/proposal_cards.jsonl line {count}.",
        submission["id"].as_str().unwrap(),
        count - 2,
        count - 1
    )));
    remove(repo, PROPOSALS, &[count - 1, count]);
    reads_and_check_pass(repo);
}
