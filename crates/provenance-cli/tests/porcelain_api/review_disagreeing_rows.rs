//! Review rows of one record that do not agree refuse each read and each
//! review write of that record with `invalid_state_file` at the line of the
//! second row. The message names the two rows and the repair. The other
//! records, the list reads and the cache catch-up continue, and the repair
//! in the message makes the record readable again.

use super::review_history::{commit, git};
use super::review_merged_decisions::{
    decide, decide_on_two_branches, fresh_list, lines, store_lines, DISPOSITIONS, PROPOSALS,
    WITHDRAWALS,
};
use super::review_merged_withdrawals::{create, pending, read, repository, withdraw, Merge};
use super::*;

/// The line number of the first line of `file` that holds `text`.
fn line_of(repo: &str, file: &str, text: &str) -> usize {
    lines(repo, file)
        .iter()
        .position(|line| line.contains(text))
        .unwrap()
        + 1
}

fn row(repo: &str, file: &str, line: usize) -> Value {
    serde_json::from_str(&lines(repo, file)[line - 1]).unwrap()
}

/// The refusal of `arguments` on `repo`.
fn refused(repo: &str, arguments: &[&str]) -> Value {
    let mut arguments = arguments.to_vec();
    arguments.extend(["--repo", repo]);
    refusal(&arguments)["error"].clone()
}

/// Each read and review write of `req_a` refuses with `expected`, and
/// `req_b`, its document, the list reads and a new record continue.
fn only_req_a_refuses(repo: &str, expected: &Value) {
    let submit = json!({"actor":"agent", "declared_by":null, "title":"Review req_a",
        "summary":"Review again.", "confidence":null, "source_ids":[], "evidence_references":[],
        "builds_on":[], "expected_revision":null, "revises":null});
    let input = directory_file(repo, "submit.json", &submit.to_string());
    let reads: [&[&str]; 3] = [
        &["api", "requirements/req_a"],
        &["api", "requirements/req_a/document"],
        &[
            "api",
            "requirements/req_a/submit",
            "--method",
            "post",
            "--input",
            &input,
        ],
    ];
    for arguments in reads {
        assert_eq!(&refused(repo, arguments), expected, "{arguments:?}");
    }
    // The target-first read gives each refusal of a state file as
    // `operation_failed` with the file and the line in its message.
    let target_first = ["req_a", "get", "--view", "review", "--format", "json"];
    let message = format!(
        "{} line {}: {}",
        expected["file"].as_str().unwrap(),
        expected["line"],
        expected["message"].as_str().unwrap()
    );
    assert_eq!(
        refused(repo, &target_first),
        json!({"kind":"operation_failed", "message":message})
    );
    read(repo, &["api", "requirements/req_b"]);
    read(repo, &["api", "requirements/req_b/document"]);
    fresh_list(repo, "dispositions");
    fresh_list(repo, "proposals");
    create(repo, "req_c");
}

fn state_file(file: &str) -> String {
    file.trim_start_matches(".provenance/state/").to_owned()
}

/// Removes line `line` of `file` and commits, as the repair says.
fn remove_line(repo: &str, file: &str, line: usize) {
    let mut stored = lines(repo, file);
    stored.remove(line - 1);
    store_lines(repo, file, &stored);
}

/// Flow: one branch approves a submission, a different branch rejects it,
/// and the merge driver merges the branches.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
#[verifies("rule_document_read_refuses_with_record", examples)]
fn decisions_with_different_results_refuse_their_record() {
    let (_directory, repo) = repository(Merge::Driver);
    let submission = decide_on_two_branches(&repo, ["accepted", "rejected"], Merge::Driver);
    let (first, second) = (row(&repo, DISPOSITIONS, 1), row(&repo, DISPOSITIONS, 2));
    let expected = json!({"kind":"invalid_state_file", "file":state_file(DISPOSITIONS), "line":2,
        "message":format!("review submission {submission} of requirement req_a has two \
            decisions with different results: {} {} at ideation/dispositions.jsonl line 1 and \
            {} {} at line 2. To repair, remove the line of the decision that you do not keep, \
            then commit.", first["decision"].as_str().unwrap(), first["id"].as_str().unwrap(),
            second["decision"].as_str().unwrap(), second["id"].as_str().unwrap())});
    only_req_a_refuses(&repo, &expected);
    let listed = fresh_list(&repo, "dispositions");
    assert_eq!(listed["data"]["items"].as_array().unwrap().len(), 2);
    let path = format!("proposals/{submission}");
    let stored = read(&repo, &["api", &path]);
    assert_eq!(stored["data"]["promotion_state"], "proposed", "{stored}");

    remove_line(&repo, DISPOSITIONS, 2);
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(state["decisions"].as_array().unwrap().len(), 1);
    assert_eq!(state["decisions"][0]["disposition"]["id"], first["id"]);
    read(&repo, &["api", "requirements/req_a/document"]);
}

/// Flow: one branch approves a submission, a different branch withdraws it,
/// and a person adds a second withdrawal by hand.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn an_approval_and_withdrawals_refuse_their_record() {
    let (_directory, repo) = repository(Merge::Driver);
    let submission = pending(&repo, "req_a");
    let first = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git(&repo, &["checkout", "-q", "-b", "side"]);
    withdraw(&repo, "req_a", &submission, "agent", "Side branch.");
    commit(&repo);
    git(&repo, &["checkout", "-q", &first]);
    decide(&repo, "req_a", &submission, "accepted", "", None);
    commit(&repo);
    git(&repo, &["merge", "--no-edit", "side"]);
    let mut withdrawals = lines(&repo, WITHDRAWALS);
    let mut copied = row(&repo, WITHDRAWALS, 1);
    copied["id"] = json!("withdrawal_by_hand");
    withdrawals.push(copied.to_string());
    store_lines(&repo, WITHDRAWALS, &withdrawals);
    let approval = row(&repo, DISPOSITIONS, 1)["id"].clone();
    let withdrawal = row(&repo, WITHDRAWALS, 1)["id"].clone();
    let expected = json!({"kind":"invalid_state_file", "file":state_file(WITHDRAWALS), "line":1,
        "message":format!("review submission {submission} of requirement req_a is accepted by \
            {} at ideation/dispositions.jsonl line 1 and withdrawn by {} at \
            ideation/withdrawals.jsonl line 1. To repair, remove the line of the approval or of \
            the withdrawal, then commit.", approval.as_str().unwrap(),
            withdrawal.as_str().unwrap())});
    only_req_a_refuses(&repo, &expected);

    remove_line(&repo, DISPOSITIONS, 1);
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(state["withdrawn"], json!([submission]));
}

/// Flow: a hand edit adds a second review submission of `req_a` with the
/// review cycle of its first submission.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn two_submissions_with_one_cycle_refuse_their_record() {
    let (_directory, repo) = repository(Merge::Driver);
    let submission = pending(&repo, "req_a");
    let first_line = line_of(&repo, PROPOSALS, &format!("\"id\":\"{submission}\""));
    let mut copied = row(&repo, PROPOSALS, first_line);
    copied["id"] = json!("submission_by_hand");
    let mut proposals = lines(&repo, PROPOSALS);
    proposals.push(copied.to_string());
    let second_line = proposals.len();
    store_lines(&repo, PROPOSALS, &proposals);
    let expected = json!({"kind":"invalid_state_file", "file":state_file(PROPOSALS),
        "line":second_line,
        "message":format!("requirement req_a has two review submissions with review cycle 1: \
            {submission} at ideation/proposal_cards.jsonl line {first_line} and \
            submission_by_hand at line {second_line}. To repair, remove the line of one \
            submission and each line of ideation/dispositions.jsonl and \
            ideation/withdrawals.jsonl that names it, then commit.")});
    only_req_a_refuses(&repo, &expected);

    // The write of req_c sorted the file again, so the line moved.
    remove_line(
        &repo,
        PROPOSALS,
        line_of(&repo, PROPOSALS, "submission_by_hand"),
    );
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(state["pending"]["proposal_id"], json!(submission));
}

/// Flow: a hand edit copies the line of a decision and changes its
/// rationale. A decision on a different record continues.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn a_decision_id_on_two_different_lines_refuses_its_record() {
    let (_directory, repo) = repository(Merge::Driver);
    let submission = pending(&repo, "req_a");
    decide(
        &repo,
        "req_a",
        &submission,
        "rejected",
        "Name the scope.",
        None,
    );
    commit(&repo);
    let mut copied = row(&repo, DISPOSITIONS, 1);
    copied["rationale"] = json!("Name the owner.");
    let mut stored = lines(&repo, DISPOSITIONS);
    stored.push(copied.to_string());
    store_lines(&repo, DISPOSITIONS, &stored);
    let expected = json!({"kind":"invalid_state_file", "file":state_file(DISPOSITIONS), "line":2,
        "message":format!("decision {} is on line 1 and line 2 of ideation/dispositions.jsonl \
            with different contents. To repair, remove the line that is not correct, then \
            commit.", copied["id"].as_str().unwrap())});
    only_req_a_refuses(&repo, &expected);
    let listed = fresh_list(&repo, "dispositions");
    assert_eq!(listed["data"]["items"], json!([]), "neither line loads");
    let other = pending(&repo, "req_b");
    decide(&repo, "req_b", &other, "deferred", "Later.", None);

    // The decision of req_b rewrote the file, so the line moved.
    remove_line(
        &repo,
        DISPOSITIONS,
        line_of(&repo, DISPOSITIONS, "Name the owner."),
    );
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(
        state["decisions"][0]["disposition"]["rationale"],
        "Name the scope."
    );
}
