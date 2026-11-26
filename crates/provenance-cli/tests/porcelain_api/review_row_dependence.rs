//! A read or a write refuses for the review rows of a record only when its
//! answer depends on those rows. A change of a different Resolution, a
//! review write addressed to a different record and a list read answer as
//! they do when the rows agree.

use super::review_cycle::{write, REVIEWER};
use super::review_disagreeing_rows::{refused, state_file};
use super::review_history::{commit, git};
use super::review_merged_decisions::{decide_on_two_branches, fresh_list, DISPOSITIONS};
use super::review_merged_withdrawals::{merge_side, pending, read, repository, withdraw, Merge};
use super::*;

/// Creates the draft Resolution `id`, which cites `req_a`.
pub(super) fn create_resolution(repo: &str, id: &str) {
    write(
        repo,
        "post",
        "resolutions",
        &json!({"id":id, "title":"One copy", "position":"Keep one copy of each record.",
            "rationale":"The owner chose one copy.", "status":"draft",
            "requirement_ids":["req_a"], "supersedes":[], "inputs":[]}),
        None,
    );
}

/// Decides the pending submission of the Resolution `id`.
pub(super) fn decide_resolution(repo: &str, id: &str, result: &str) {
    let path = format!("resolutions/{id}");
    let submission = read(repo, &["api", &path])["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let accepted = result == "accepted";
    let body = json!({
        "actor":{"identity_type":"human","id":REVIEWER}, "decision":result,
        "rationale":(!accepted).then_some("Not yet."),
        "canonical_artifact":accepted
            .then(|| json!({"artifact_type":"resolution","artifact_id":id})),
        "feedback":null, "declared_by":null
    });
    write(
        repo,
        "post",
        &format!("{path}/submissions/{submission}/decide"),
        &body,
        None,
    );
}

/// Commits, then one branch approves the pending submission of the
/// Resolution `id` and a second branch rejects it, and the merge driver
/// merges the branches. The review rows of `id` then disagree.
pub(super) fn decide_resolution_on_two_branches(repo: &str, id: &str) {
    commit(repo);
    let first = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git(repo, &["checkout", "-q", "-b", "side"]);
    decide_resolution(repo, id, "rejected");
    commit(repo);
    git(repo, &["checkout", "-q", &first]);
    decide_resolution(repo, id, "accepted");
    commit(repo);
    merge_side(repo, Merge::Driver);
}

/// The arguments of a patch of `path` on `repo` with `body` and the current
/// etag.
pub(super) fn patch(repo: &str, path: &str, body: &Value) -> Vec<String> {
    let etag = read(repo, &["api", path])["data"]["edit"]["etag"]
        .as_str()
        .unwrap()
        .to_owned();
    let input = directory_file(repo, "patch.json", &body.to_string());
    let header = format!("If-Match: {etag}");
    [
        "api", path, "--method", "patch", "--input", &input, "--header", &header, "--repo", repo,
    ]
    .map(str::to_owned)
    .to_vec()
}

/// Flow: a draft Rule cites the Resolutions `res_x` and `res_a`. One branch
/// approves the submission of `res_a`, and the other branch rejects it. A
/// change of `res_x` does not need the review state of `res_a`, so it
/// passes. The change of the Rule to active needs the approval of each
/// Resolution that it cites, so it gives the refusal of `res_a`.
#[test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
fn a_change_of_a_resolution_does_not_refuse_for_a_second_resolution() {
    let (_directory, repo) = repository(Merge::Driver);
    create_resolution(&repo, "res_a");
    create_resolution(&repo, "res_x");
    write(
        &repo,
        "post",
        "rules",
        &json!({"id":"rule_d", "statement":"The system keeps one copy.",
            "severity":"medium", "status":"draft", "requirement_ids":["req_a"],
            "resolution_ids":["res_x", "res_a"]}),
        None,
    );
    decide_resolution_on_two_branches(&repo, "res_a");
    let fault = refused(&repo, &["api", "resolutions/res_a"]);
    assert_eq!(fault["kind"], "invalid_state_file", "{fault}");

    let title = patch(&repo, "resolutions/res_x", &json!({"title":"Two copies"}));
    success(&title.iter().map(String::as_str).collect::<Vec<_>>());
    decide_resolution(&repo, "res_x", "accepted");
    // The decision of res_x rewrote the file, so the lines of res_a moved.
    let fault = refused(&repo, &["api", "resolutions/res_a"]);
    let active = patch(&repo, "rules/rule_d", &json!({"status":"active"}));
    let active = active.iter().map(String::as_str).collect::<Vec<_>>();
    assert_eq!(refusal(&active)["error"], fault);
    let rule = read(&repo, &["api", "rules/rule_d"]);
    assert_eq!(rule["data"]["status"], "draft", "{rule}");
}

/// Flow: one branch approves the submission S of `req_a`, and the other
/// branch rejects it. A decision or a withdrawal of S that is addressed to
/// `req_b` gives the refusal of the address: the lines of S name `req_a`.
#[test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
fn a_review_write_addressed_to_a_different_record_gives_the_address_refusal() {
    let (_directory, repo) = repository(Merge::Driver);
    let submission = decide_on_two_branches(&repo, ["accepted", "rejected"], Merge::Driver);
    let addressed = format!("requirements/req_b/submissions/{submission}");
    let expected = json!({"kind":"invalid_update",
        "message":"the review submission does not belong to the addressed record"});
    let decision = super::review_cycle::decision("accepted", "req_b", None);
    let withdrawal = json!({"actor":{"identity_type":"agent","id":"agent"}, "declared_by":null, "reason":"Again."});
    for (action, body) in [("decide", decision), ("withdraw", withdrawal)] {
        let input = directory_file(&repo, "write.json", &body.to_string());
        let path = format!("{addressed}/{action}");
        let arguments = ["api", &path, "--method", "post", "--input", &input];
        assert_eq!(refused(&repo, &arguments), expected, "{action}");
    }
}

/// Flow: one branch approves the submission S of `req_a`, and the other
/// branch withdraws it. The list of Proposals does not take the state of S
/// from these rows: S keeps its stored state.
#[test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
fn the_list_read_keeps_the_stored_state_of_an_accepted_and_withdrawn_submission() {
    let (_directory, repo) = repository(Merge::Driver);
    let submission = pending(&repo, "req_a");
    let first = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git(&repo, &["checkout", "-q", "-b", "side"]);
    withdraw(&repo, "req_a", &submission, "agent", "Side branch.");
    commit(&repo);
    git(&repo, &["checkout", "-q", &first]);
    super::review_merged_decisions::decide(&repo, "req_a", &submission, "accepted", "", None);
    commit(&repo);
    merge_side(&repo, Merge::Driver);
    let fault = refused(&repo, &["api", "requirements/req_a"]);
    assert_eq!(
        fault["file"],
        state_file(super::review_merged_decisions::WITHDRAWALS)
    );

    let listed = fresh_list(&repo, "proposals");
    let item = listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == json!(submission))
        .cloned()
        .unwrap();
    assert_eq!(item["promotion_state"], "proposed", "{item}");
}

/// Flow: the manifest names two reviewers. One approves the submission S
/// of `req_a`, and a second line approves it again with the other actor and
/// a different rationale. The result
/// is the decision, so S reads as accepted and `check` passes.
#[test]
#[verifies("rule_same_result_decisions_read_as_one", examples)]
fn two_approvals_by_different_actors_read_as_one() {
    let (_directory, repo) = repository(Merge::Driver);
    let actors = ["--disposition-actor-id", REVIEWER, "--disposition-actor-id"];
    let init = [
        "init",
        "--path",
        &repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ];
    success(&[&init[..], &actors[..], &["second_reviewer"]].concat());
    let submission = pending(&repo, "req_a");
    super::review_merged_decisions::decide(&repo, "req_a", &submission, "accepted", "", None);
    commit(&repo);
    let mut stored = super::review_merged_decisions::lines(&repo, DISPOSITIONS);
    let mut second: Value = serde_json::from_str(&stored[0]).unwrap();
    second["id"] = json!("approval_by_hand");
    second["actor"] = json!({"identity_type":"human", "id":"second_reviewer"});
    second["rationale"] = json!("The scope is clear.");
    stored.push(second.to_string());
    super::review_merged_decisions::store_lines(&repo, DISPOSITIONS, &stored);

    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(state["pending"], Value::Null, "{state}");
    assert_eq!(state["decisions"].as_array().unwrap().len(), 2, "{state}");
    assert_eq!(
        state["current_acceptance"]["disposition"]["proposal_id"],
        json!(submission)
    );
    super::review_merged_withdrawals::check_passes(&repo);
}
