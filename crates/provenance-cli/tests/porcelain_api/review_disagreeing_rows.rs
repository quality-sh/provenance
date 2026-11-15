//! Review rows of one record that do not agree refuse each read and each
//! review write of that record with `invalid_state_file` at the line of the
//! second row. The message names the rows and their lines, and the hint
//! gives the repair, which names each line that it removes. The other records, the list reads and the cache catch-up
//! continue, `provenance check` reports the same fault, and after the person
//! removes the named lines and commits, the reads and `check` pass.

use super::review_history::{commit, git};
use super::review_merged_decisions::{
    decide, decide_on_two_branches, fresh_list, lines, store_lines, DISPOSITIONS, PROPOSALS,
    WITHDRAWALS,
};
use super::review_merged_withdrawals::{
    check_passes, create, pending, read, repository, withdraw, Merge,
};
use super::*;

/// The line number of the first line of `file` that holds `text`.
pub(super) fn line_of(repo: &str, file: &str, text: &str) -> usize {
    lines(repo, file)
        .iter()
        .position(|line| line.contains(text))
        .unwrap()
        + 1
}

pub(super) fn row(repo: &str, file: &str, line: usize) -> Value {
    serde_json::from_str(&lines(repo, file)[line - 1]).unwrap()
}

/// The refusal of `arguments` on `repo`.
pub(super) fn refused(repo: &str, arguments: &[&str]) -> Value {
    let mut arguments = arguments.to_vec();
    arguments.extend(["--repo", repo]);
    refusal(&arguments)["error"].clone()
}

/// Each read and review write of `req_a` refuses with `expected`, and
/// `req_b`, its document, the list reads and a new record continue.
pub(super) fn only_req_a_refuses(repo: &str, expected: &Value) {
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
    read(repo, &["api", "requirements/req_b"]);
    read(repo, &["api", "requirements/req_b/document"]);
    fresh_list(repo, "dispositions");
    fresh_list(repo, "proposals");
    create(repo, "req_c");
}

/// The repair of a row ID on lines that do not agree.
pub(super) const KEEP_ONE_LINE: &str =
    "To repair, keep one of these lines and remove the others, then commit.";

pub(super) fn state_file(file: &str) -> String {
    file.trim_start_matches(".provenance/state/").to_owned()
}

/// `provenance check` exits 1, and its one review row finding is the fault
/// of `expected`, the refusal of a read, with its file and its line.
pub(super) fn check_reports(repo: &str, expected: &Value) {
    check_reports_all(repo, std::slice::from_ref(expected));
}

/// `provenance check` exits 1, and its review row findings are the faults of
/// `expected` in that order, each the refusal of a read with its file and
/// its line.
pub(super) fn check_reports_all(repo: &str, expected: &[Value]) {
    let result = output(&["check", "--repo", repo]);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(1), "{stderr}");
    let findings = expected.iter().map(|fault| {
        format!(
            "{} line {}: {}; {}",
            fault["file"].as_str().unwrap(),
            fault["line"],
            fault["message"].as_str().unwrap(),
            fault["hint"].as_str().unwrap()
        )
    });
    assert_eq!(
        crate::review_row_findings::review_row_findings(&result),
        findings.collect::<Vec<_>>()
    );
}

/// Removes the lines that `remove` names, file by file, and commits once,
/// as the repair in a refusal says.
pub(super) fn remove_lines(repo: &str, remove: &[(&str, usize)]) {
    let mut files = remove.iter().map(|(file, _)| *file).collect::<Vec<_>>();
    files.sort_unstable();
    files.dedup();
    for file in files {
        let removed = remove
            .iter()
            .filter(|(named, _)| *named == file)
            .map(|(_, line)| *line)
            .collect::<Vec<_>>();
        let mut text = String::new();
        for (index, line) in lines(repo, file).iter().enumerate() {
            if !removed.contains(&(index + 1)) {
                text.push_str(line);
                text.push('\n');
            }
        }
        directory_file(repo, file, &text);
    }
    super::review_history::commit(repo);
}

/// The ID and the result of the decision at `line`, as `rejected D1`.
fn decision_at(repo: &str, line: usize) -> String {
    let decision = row(repo, DISPOSITIONS, line);
    format!(
        "{} {}",
        decision["decision"].as_str().unwrap(),
        decision["id"].as_str().unwrap()
    )
}

/// Flow: one branch approves a submission, a different branch rejects it,
/// and the merge driver merges the branches. The repair removes the line of
/// the result that the person does not keep.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
#[verifies("rule_document_read_refuses_with_record", examples)]
#[verifies("rule_check_reports_review_row_faults", examples)]
fn decisions_with_different_results_refuse_their_record() {
    let (_directory, repo) = repository(Merge::Driver);
    let submission = decide_on_two_branches(&repo, ["accepted", "rejected"], Merge::Driver);
    let expected = json!({"kind":"invalid_state_file", "file":state_file(DISPOSITIONS), "line":2,
        "message":format!("review submission {submission} of requirement req_a has decisions \
            with different results in ideation/dispositions.jsonl: {} at line 1 and {} at line \
            2", decision_at(&repo, 1), decision_at(&repo, 2)),
        "hint":"To repair, remove each of these lines that has a result that you do not keep, \
            then commit."});
    only_req_a_refuses(&repo, &expected);
    check_reports(&repo, &expected);
    let listed = fresh_list(&repo, "dispositions");
    assert_eq!(listed["data"]["items"].as_array().unwrap().len(), 2);
    let path = format!("proposals/{submission}");
    let stored = read(&repo, &["api", &path]);
    assert_eq!(stored["data"]["promotion_state"], "proposed", "{stored}");

    let kept = row(&repo, DISPOSITIONS, 1)["id"].clone();
    remove_lines(&repo, &[(DISPOSITIONS, 2)]);
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(state["decisions"].as_array().unwrap().len(), 1);
    assert_eq!(state["decisions"][0]["disposition"]["id"], kept);
    read(&repo, &["api", "requirements/req_a/document"]);
    check_passes(&repo);
}

/// Flow: one branch approves a submission, a different branch withdraws it,
/// and a person adds a second withdrawal by hand. The repair removes each
/// withdrawal line that the refusal names.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
#[verifies("rule_check_reports_review_row_faults", examples)]
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
        "message":format!("review submission {submission} of requirement req_a is accepted and \
            withdrawn. The approvals in ideation/dispositions.jsonl: {} at line 1. The \
            withdrawals in ideation/withdrawals.jsonl: {} at line 1 and withdrawal_by_hand at \
            line 2", approval.as_str().unwrap(), withdrawal.as_str().unwrap()),
        "hint":"To repair, remove each approval line or each withdrawal line, then commit."});
    only_req_a_refuses(&repo, &expected);
    check_reports(&repo, &expected);

    remove_lines(&repo, &[(WITHDRAWALS, 1), (WITHDRAWALS, 2)]);
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(state["withdrawn"], json!([]));
    assert_eq!(state["current_acceptance"]["disposition"]["id"], approval);
    check_passes(&repo);
}

/// The message of two submissions of `req_a` with review cycle 1, with the
/// lines of each.
fn one_cycle_message(repo: &str, submissions: [&str; 2]) -> (Value, usize) {
    let line = |submission: &str| line_of(repo, PROPOSALS, &format!("\"id\":\"{submission}\""));
    let (first, second) = (line(submissions[0]), line(submissions[1]));
    let expected = json!({"kind":"invalid_state_file", "file":state_file(PROPOSALS),
        "line":second,
        "message":format!("requirement req_a has 2 review submissions with review cycle 1. To \
            repair, keep one submission and remove the lines of each other submission, then \
            commit. The lines of {}: ideation/proposal_cards.jsonl line {first}. The lines of \
            {}: ideation/proposal_cards.jsonl line {second}.", submissions[0], submissions[1])});
    (expected, second)
}

/// Flow: a hand edit adds a second review submission of `req_a` with the
/// review cycle of its first submission.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
#[verifies("rule_check_reports_review_row_faults", examples)]
fn two_submissions_with_one_cycle_refuse_their_record() {
    let (_directory, repo) = repository(Merge::Driver);
    let submission = pending(&repo, "req_a");
    let first_line = line_of(&repo, PROPOSALS, &format!("\"id\":\"{submission}\""));
    let mut copied = row(&repo, PROPOSALS, first_line);
    copied["id"] = json!("submission_by_hand");
    let mut proposals = lines(&repo, PROPOSALS);
    proposals.push(copied.to_string());
    store_lines(&repo, PROPOSALS, &proposals);
    let (expected, _) = one_cycle_message(&repo, [&submission, "submission_by_hand"]);
    only_req_a_refuses(&repo, &expected);

    // The write of req_c sorted the file again, so the lines moved.
    let (expected, by_hand) = one_cycle_message(&repo, [&submission, "submission_by_hand"]);
    check_reports(&repo, &expected);
    remove_lines(&repo, &[(PROPOSALS, by_hand)]);
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(state["pending"]["proposal_id"], json!(submission));
    check_passes(&repo);
}

/// Flow: a reviewer rejects a submission, the agent withdraws it, and a hand
/// edit copies the line of the decision and changes its rationale. The
/// withdrawal names a submission whose decision refuses, and a decision on
/// a different record continues.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
#[verifies("rule_check_reports_review_row_faults", examples)]
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
    withdraw(&repo, "req_a", &submission, "agent", "Start again.");
    commit(&repo);
    let mut copied = row(&repo, DISPOSITIONS, 1);
    copied["rationale"] = json!("Name the owner.");
    let mut stored = lines(&repo, DISPOSITIONS);
    stored.push(copied.to_string());
    store_lines(&repo, DISPOSITIONS, &stored);
    let message = |first: usize, second: usize| {
        format!(
            "decision {} is on lines {first} and {second} of ideation/dispositions.jsonl with \
             different contents",
            copied["id"].as_str().unwrap()
        )
    };
    let expected = json!({"kind":"invalid_state_file", "file":state_file(DISPOSITIONS), "line":2,
        "message":message(1, 2), "hint":KEEP_ONE_LINE});
    only_req_a_refuses(&repo, &expected);
    let listed = fresh_list(&repo, "dispositions");
    assert_eq!(listed["data"]["items"], json!([]), "neither line loads");
    let other = pending(&repo, "req_b");
    decide(&repo, "req_b", &other, "deferred", "Later.", None);

    // The decision of req_b rewrote the file, so the lines moved.
    let (first, second) = (
        line_of(&repo, DISPOSITIONS, "Name the scope."),
        line_of(&repo, DISPOSITIONS, "Name the owner."),
    );
    let expected = json!({"kind":"invalid_state_file", "file":state_file(DISPOSITIONS),
        "line":second, "message":message(first, second), "hint":KEEP_ONE_LINE});
    check_reports(&repo, &expected);
    remove_lines(&repo, &[(DISPOSITIONS, second)]);
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(
        state["decisions"][0]["disposition"]["rationale"],
        "Name the scope."
    );
    assert_eq!(state["withdrawn"], json!([submission]));
    check_passes(&repo);
}
