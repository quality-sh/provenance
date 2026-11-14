//! Two branches each decide one review submission, and Git merges the
//! branches. A hand edit copies one review row to a second line. The review
//! rows that say the same thing read as one, and the reads, the writes, the
//! cache catch-up and `provenance check` continue.

use super::review_cycle::{init_with_reviewer, REVIEWER};
use super::review_history::{commit, git};
use super::review_merged_withdrawals::{
    check_passes, create, merge_side, pending, read, repository, withdraw, Merge,
};
use super::*;

pub(super) const DISPOSITIONS: &str = ".provenance/state/scopes/default/ideation/dispositions.jsonl";
pub(super) const WITHDRAWALS: &str = ".provenance/state/scopes/default/ideation/withdrawals.jsonl";
pub(super) const PROPOSALS: &str = ".provenance/state/scopes/default/ideation/proposal_cards.jsonl";

/// Decides one submission of `record` as the configured reviewer, with
/// `rationale` and `feedback` as the person wrote them.
pub(super) fn decide(
    repo: &str,
    record: &str,
    submission: &str,
    result: &str,
    rationale: &str,
    feedback: Option<&str>,
) {
    let accepted = result == "accepted";
    let body = json!({
        "actor":{"identity_type":"human","id":REVIEWER}, "decision":result,
        "rationale":(!accepted).then_some(rationale),
        "canonical_artifact":accepted
            .then(|| json!({"artifact_type":"requirement","artifact_id":record})),
        "feedback":feedback.map(|body| json!({"role":"user","body":body})),
        "declared_by":null
    });
    let path = format!("requirements/{record}/submissions/{submission}/decide");
    super::review_cycle::write(repo, "post", &path, &body, None);
}

/// Each branch decides the pending submission of `req_a` with `results`,
/// the first branch first, and Git merges the side branch in the way
/// `merge` names. Gives the submission.
pub(super) fn decide_on_two_branches(repo: &str, results: [&str; 2], merge: Merge) -> String {
    let submission = pending(repo, "req_a");
    let first = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git(repo, &["checkout", "-q", "-b", "side"]);
    let feedback = Some("Name the scope on the side branch.");
    decide(repo, "req_a", &submission, results[1], "Side.", feedback);
    commit(repo);
    git(repo, &["checkout", "-q", &first]);
    decide(repo, "req_a", &submission, results[0], "First.", None);
    commit(repo);
    merge_side(repo, merge);
    submission
}

/// The stored lines of one state file.
pub(super) fn lines(repo: &str, file: &str) -> Vec<String> {
    let text = std::fs::read_to_string(std::path::Path::new(repo).join(file)).unwrap();
    text.lines().map(str::to_owned).collect()
}

/// Writes `lines` to one state file and commits it.
pub(super) fn store_lines(repo: &str, file: &str, lines: &[String]) {
    let text = lines.iter().map(|line| format!("{line}\n")).collect::<String>();
    directory_file(repo, file, &text);
    commit(repo);
}

/// A list read whose cache catch-up loaded the changed state.
pub(super) fn fresh_list(repo: &str, path: &str) -> Value {
    let listed = read(repo, &["api", path]);
    let meta = &listed["meta"];
    assert_eq!(meta["freshness_error"], Value::Null, "{listed}");
    assert_ne!(meta["stamp"]["policy"], "catch_up_failed", "{listed}");
    listed
}

/// Submits `req_a` again as the answer to the rejection of `submission`.
fn resubmit(repo: &str, submission: &str) -> Value {
    let body = json!({"actor":"agent", "declared_by":null, "title":"Review req_a",
        "summary":"Name the scope.", "confidence":null, "source_ids":[],
        "evidence_references":[], "builds_on":[], "expected_revision":null,
        "revises":submission});
    super::review_cycle::write(repo, "post", "requirements/req_a/submit", &body, None)
}

fn decisions_with_one_result_read_as_one(result: &str, merge: Merge) {
    let (_directory, repo) = repository(merge);
    let submission = decide_on_two_branches(&repo, [result, result], merge);

    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    let history = state["decisions"].as_array().unwrap();
    assert_eq!(history.len(), 2, "{state}");
    let ids = history
        .iter()
        .map(|row| row["disposition"]["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "the history is sorted by the row ID");
    assert!(history
        .iter()
        .all(|row| row["disposition"]["decision"] == result));
    assert_eq!(state["pending"], Value::Null);
    if result == "accepted" {
        assert_eq!(state["current_acceptance"]["disposition"]["id"], ids[0]);
    }
    let view = read(&repo, &["req_a", "get", "--view", "review", "--format", "json"]);
    assert_eq!(view["review"]["decision"], state);
    read(&repo, &["api", "requirements/req_a/document"]);
    let path = format!("proposals/{submission}");
    assert_eq!(read(&repo, &["api", &path])["data"]["promotion_state"], result);
    let listed = fresh_list(&repo, "dispositions");
    assert_eq!(listed["data"]["items"].as_array().unwrap().len(), 2);
    check_passes(&repo);

    create(&repo, "req_c");
    if result == "rejected" {
        let resubmitted = resubmit(&repo, &submission);
        assert_eq!(resubmitted["data"]["fact"], "submitted");
        let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
        assert_eq!(state["pending"]["revises"], json!(submission));
    }
    check_passes(&repo);
}

/// Flow: two branches approve one submission, with other comments, and the
/// merge driver merges them.
#[test]
#[verifies("rule_same_result_decisions_read_as_one", examples)]
#[verifies("rule_review_history_lists_each_row_once", examples)]
fn two_approvals_merged_by_the_driver_read_as_one() {
    decisions_with_one_result_read_as_one("accepted", Merge::Driver);
}

/// Flow: two branches reject one submission, the person keeps both sides
/// of the conflict, and the record takes a resubmission.
#[test]
#[verifies("rule_same_result_decisions_read_as_one", examples)]
#[verifies("rule_review_history_lists_each_row_once", examples)]
fn two_rejections_kept_by_a_person_read_as_one() {
    decisions_with_one_result_read_as_one("rejected", Merge::KeepBoth);
}

/// Flow: two branches defer one submission, and the merge driver merges
/// them.
#[test]
#[verifies("rule_same_result_decisions_read_as_one", examples)]
fn two_deferrals_merged_by_the_driver_read_as_one() {
    decisions_with_one_result_read_as_one("deferred", Merge::Driver);
}

/// Flow: a hand edit copies the line of a submission, of a decision and of
/// a withdrawal. Each row reads one time, the catch-up loads it one time,
/// and `check` passes.
#[test]
#[verifies("rule_equal_review_row_lines_read_as_one", examples)]
#[verifies("rule_repeated_withdrawals_read_as_one", examples)]
fn copied_review_row_lines_read_as_one() {
    let (_directory, repo) = init_with_reviewer();
    create(&repo, "req_a");
    create(&repo, "req_b");
    let submission = pending(&repo, "req_a");
    decide(&repo, "req_a", &submission, "rejected", "Name the scope.", None);
    withdraw(&repo, "req_a", &submission, "agent", "Start again.");
    commit(&repo);
    let before = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    fresh_list(&repo, "dispositions");
    fresh_list(&repo, "proposals");

    for file in [DISPOSITIONS, WITHDRAWALS, PROPOSALS] {
        let mut stored = lines(&repo, file);
        let copied = stored[0].clone();
        stored.push(copied);
        store_lines(&repo, file, &stored);
    }

    let after = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(after, before);
    assert_eq!(after["decisions"].as_array().unwrap().len(), 1);
    assert_eq!(after["withdrawn"], json!([submission]));
    let listed = fresh_list(&repo, "dispositions");
    assert_eq!(listed["data"]["items"].as_array().unwrap().len(), 1);
    let proposals = fresh_list(&repo, "proposals")["data"]["items"].clone();
    let mut ids = proposals
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count, "{proposals}");
    read(&repo, &["api", "requirements/req_a/document"]);
    check_passes(&repo);
    create(&repo, "req_c");
    let other = pending(&repo, "req_b");
    decide(&repo, "req_b", &other, "accepted", "", None);
    check_passes(&repo);
}
