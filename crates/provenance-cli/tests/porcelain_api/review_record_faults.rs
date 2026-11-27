//! The rows that name a review row that refuses are a part of the fault of
//! its record: the other records of the scope read and take writes, and a
//! write that needs the review state of the record gives the refusal of
//! that record. Review rows that agree beside a withdrawal or a newer
//! submission read as before.

use super::review_cycle::describe;
use super::review_disagreeing_rows::{
    check_reports, check_reports_all, line_of, only_req_a_refuses, refused, remove_lines, row,
    state_file, KEEP_ONE_LINE,
};
use super::review_history::{commit, git};
use super::review_merged_decisions::{
    decide, fresh_list, lines, store_lines, DISPOSITIONS, PROPOSALS, WITHDRAWALS,
};
use super::review_merged_withdrawals::{
    check_passes, create, merge_side, pending, read, repository, withdraw, Merge,
};
use super::review_row_dependence::{create_resolution, decide_resolution_on_two_branches};
use super::*;

/// The line of the row of `file` whose `field` is `id`.
fn line_naming(repo: &str, file: &str, field: &str, id: &str) -> usize {
    line_of(repo, file, &format!("\"{field}\":\"{id}\""))
}

/// The refusal of a write of `body` to `path`.
fn refused_write(repo: &str, method: &str, path: &str, body: &Value) -> Value {
    let input = directory_file(repo, "write.json", &body.to_string());
    refused(repo, &["api", path, "--method", method, "--input", &input])
}

/// The refusal of the submission `submission` on two lines that disagree,
/// at the lines that hold it now.
fn submission_on_lines(repo: &str, submission: &str) -> Value {
    let named = format!("\"id\":\"{submission}\"");
    let holding = lines(repo, PROPOSALS)
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(&named))
        .map(|(index, _)| index + 1)
        .collect::<Vec<_>>();
    let (first, second) = (holding[0], holding[1]);
    json!({"kind":"invalid_state_file", "file":state_file(PROPOSALS), "line":second,
        "message":format!("review submission {submission} is on lines {first} and {second} of \
            ideation/proposal_cards.jsonl with different contents"), "hint":KEEP_ONE_LINE})
}

/// Flow: a reviewer rejects the submission of `req_a`, the agent withdraws
/// it, and a hand edit copies the line of the submission and changes its
/// summary. The decision and the withdrawal name a submission whose ID is on
/// lines that disagree: only `req_a` refuses, also for a decision or a
/// withdrawal of that submission.
#[test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_check_reports_review_row_faults", examples)]
fn a_submission_id_on_lines_that_disagree_refuses_only_its_record() {
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
    let first = line_naming(&repo, PROPOSALS, "id", &submission);
    let mut changed = row(&repo, PROPOSALS, first);
    changed["summary"] = json!("Changed by hand.");
    let mut proposals = lines(&repo, PROPOSALS);
    proposals.push(changed.to_string());
    store_lines(&repo, PROPOSALS, &proposals);
    let expected = |repo: &str| submission_on_lines(repo, &submission);
    // The other records of the scope continue.
    read(&repo, &["api", "requirements/req_b/document"]);
    fresh_list(&repo, "proposals");
    fresh_list(&repo, "dispositions");
    create(&repo, "req_d");
    let addressed = format!("requirements/req_a/submissions/{submission}");
    let decision = super::review_cycle::decision("accepted", "req_a", None);
    assert_eq!(
        refused_write(&repo, "post", &format!("{addressed}/decide"), &decision),
        expected(&repo)
    );
    let withdrawal = json!({"actor":{"identity_type":"agent","id":"agent"}, "declared_by":null, "reason":"Again."});
    assert_eq!(
        refused_write(&repo, "post", &format!("{addressed}/withdraw"), &withdrawal),
        expected(&repo)
    );
    only_req_a_refuses(&repo, &expected(&repo));
    check_reports(&repo, &expected(&repo));

    let second = expected(&repo)["line"].as_u64().unwrap();
    remove_lines(&repo, &[(PROPOSALS, usize::try_from(second).unwrap())]);
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(state["withdrawn"], json!([submission]));
    assert_eq!(state["decisions"].as_array().unwrap().len(), 1);
    check_passes(&repo);
}

/// Flow: one branch approves the submission S of `req_a`, the other branch
/// withdraws it, and a hand edit copies the line of S and changes its
/// summary. The record refuses with the fault of the ID, and one run of
/// `check` also names the approval with the withdrawal. After the repair
/// step of each, the read and `check` pass.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_check_reports_review_row_faults", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn an_approval_and_a_withdrawal_of_a_submission_on_lines_that_disagree() {
    let (_directory, repo) = repository(Merge::Driver);
    let submission = pending(&repo, "req_a");
    let first = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git(&repo, &["checkout", "-q", "-b", "side"]);
    withdraw(&repo, "req_a", &submission, "agent", "Side branch.");
    commit(&repo);
    git(&repo, &["checkout", "-q", &first]);
    decide(&repo, "req_a", &submission, "accepted", "", None);
    commit(&repo);
    merge_side(&repo, Merge::Driver);
    let line = line_naming(&repo, PROPOSALS, "id", &submission);
    let mut changed = row(&repo, PROPOSALS, line);
    changed["summary"] = json!("Changed by hand.");
    let mut proposals = lines(&repo, PROPOSALS);
    proposals.push(changed.to_string());
    store_lines(&repo, PROPOSALS, &proposals);
    let approval = row(&repo, DISPOSITIONS, 1)["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let withdrawal = row(&repo, WITHDRAWALS, 1)["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let withdrawn = json!({"kind":"invalid_state_file", "file":state_file(WITHDRAWALS), "line":1,
        "message":format!("review submission {submission} of requirement req_a is accepted and \
            withdrawn. The approvals in ideation/dispositions.jsonl: {approval} at line 1. The \
            withdrawals in ideation/withdrawals.jsonl: {withdrawal} at line 1"),
        "hint":"To repair, remove each approval line or each withdrawal line, then commit."});
    only_req_a_refuses(&repo, &submission_on_lines(&repo, &submission));
    // The write of req_c sorted the file again, so the lines moved.
    let on_lines = submission_on_lines(&repo, &submission);
    let copied = usize::try_from(on_lines["line"].as_u64().unwrap()).unwrap();
    check_reports_all(&repo, &[on_lines, withdrawn]);

    remove_lines(&repo, &[(PROPOSALS, copied), (WITHDRAWALS, 1)]);
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(state["withdrawn"], json!([]));
    assert_eq!(state["current_acceptance"]["disposition"]["id"], approval);
    check_passes(&repo);
}

/// The lines of one submission as the refusal names them: its line, then
/// the line of each decision that names it.
fn submission_lines(repo: &str, submission: &str) -> String {
    let proposal = line_naming(repo, PROPOSALS, "id", submission);
    let decision = line_naming(repo, DISPOSITIONS, "proposal_id", submission);
    format!(
        "ideation/proposal_cards.jsonl line {proposal}; ideation/dispositions.jsonl line \
         {decision}"
    )
}

/// Flow (pairs 15 and 16): each branch changes `req_a`, which opens a
/// review submission with review cycle 2 on each branch. One branch
/// approves its submission and the other rejects its submission. The merge
/// driver refuses the two changes of the record and the two submissions with
/// one cycle. The person keeps the record of the first branch and the
/// submission lines of both branches. The repair removes each line of the
/// rejected submission.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
#[verifies("rule_check_reports_review_row_faults", examples)]
fn two_changes_of_one_record_merged_give_two_submissions_with_one_cycle() {
    let (_directory, repo) = repository(Merge::Driver);
    let first = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    git(&repo, &["checkout", "-q", "-b", "side"]);
    describe(&repo, "req_a", "The change of the side branch.");
    let rejected = pending(&repo, "req_a");
    decide(&repo, "req_a", &rejected, "rejected", "Side.", None);
    commit(&repo);
    git(&repo, &["checkout", "-q", &first]);
    describe(&repo, "req_a", "The change of the first branch.");
    let accepted = pending(&repo, "req_a");
    decide(&repo, "req_a", &accepted, "accepted", "", None);
    commit(&repo);
    let merged = super::review_history::git_output(&repo, &["merge", "--no-edit", "side"]);
    assert!(!merged.status.success(), "{merged:?}");
    let conflicts = git(&repo, &["diff", "--name-only", "--diff-filter=U"]);
    assert!(
        conflicts.lines().any(|file| file == PROPOSALS),
        "{conflicts}"
    );
    for file in conflicts.lines().filter(|file| *file != PROPOSALS) {
        git(&repo, &["checkout", "--ours", "--", file]);
    }
    let ours = git(&repo, &["show", &format!(":2:{PROPOSALS}")]);
    let theirs = git(&repo, &["show", &format!(":3:{PROPOSALS}")]);
    let mut kept = ours.lines().map(str::to_owned).collect::<Vec<_>>();
    let added = theirs
        .lines()
        .filter(|line| !ours.lines().any(|our| our == *line));
    kept.extend(added.map(str::to_owned));
    store_lines(&repo, PROPOSALS, &kept);

    let mut submissions = [accepted.clone(), rejected.clone()];
    submissions.sort_by_key(|submission| line_naming(&repo, PROPOSALS, "id", submission));
    let second = line_naming(&repo, PROPOSALS, "id", &submissions[1]);
    let expected = json!({"kind":"invalid_state_file", "file":state_file(PROPOSALS),
        "line":second,
        "message":format!("requirement req_a has 2 review submissions with review cycle 2. The \
            lines of {}: {}. The lines of {}: {}", submissions[0],
            submission_lines(&repo, &submissions[0]), submissions[1],
            submission_lines(&repo, &submissions[1])),
        "hint":"To repair, keep one submission and remove the lines of each other submission, \
            then commit."});
    assert_eq!(refused(&repo, &["api", "requirements/req_a"]), expected);
    check_reports(&repo, &expected);
    read(&repo, &["api", "requirements/req_b/document"]);

    remove_lines(
        &repo,
        &[
            (PROPOSALS, line_naming(&repo, PROPOSALS, "id", &rejected)),
            (
                DISPOSITIONS,
                line_naming(&repo, DISPOSITIONS, "proposal_id", &rejected),
            ),
        ],
    );
    let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
    assert_eq!(
        state["current_acceptance"]["disposition"]["proposal_id"],
        json!(accepted)
    );
    read(&repo, &["api", "requirements/req_a/document"]);
    check_passes(&repo);
}

/// Flow (pairs 12, 13, 19 and 20): one branch withdraws the submission of
/// `req_a` or decides it, and the other branch decides it, withdraws it or
/// changes `req_a`. These rows agree: the reads show each row, and `check`
/// passes. No Rule binds this test: it shows that these states read as on
/// the main branch.
#[test]
fn rows_beside_a_withdrawal_or_a_newer_submission_read_as_before() {
    for (side, first) in [
        ("withdraw", "rejected"),
        ("withdraw", "deferred"),
        ("withdraw", "change"),
        ("rejected", "change"),
    ] {
        let (_directory, repo) = repository(Merge::Driver);
        let submission = pending(&repo, "req_a");
        let branch = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
        git(&repo, &["checkout", "-q", "-b", "side"]);
        for (step, actor) in [(side, "side"), (first, "first")] {
            match step {
                "withdraw" => withdraw(&repo, "req_a", &submission, "agent", actor),
                "change" => {
                    describe(
                        &repo,
                        "req_a",
                        &format!("The change of the {actor} branch."),
                    );
                }
                result => decide(&repo, "req_a", &submission, result, actor, None),
            }
            commit(&repo);
            if actor == "side" {
                git(&repo, &["checkout", "-q", &branch]);
            }
        }
        merge_side(&repo, Merge::Driver);

        let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
        let decided = [side, first]
            .iter()
            .filter(|step| !matches!(**step, "withdraw" | "change"))
            .count();
        assert_eq!(
            state["decisions"].as_array().unwrap().len(),
            decided,
            "{state}"
        );
        if side == "withdraw" {
            assert_eq!(state["withdrawn"], json!([submission]), "{state}");
        }
        if first == "change" {
            assert_ne!(
                state["pending"]["proposal_id"],
                json!(submission),
                "{state}"
            );
            assert!(state["pending"]["proposal_id"].is_string(), "{state}");
        }
        read(&repo, &["api", "requirements/req_a/document"]);
        check_passes(&repo);
    }
}

/// Flow: a Rule in review cites the Resolution `res_a`. One branch approves
/// the submission of `res_a`, and the other branch rejects it. The change
/// of the Rule to active needs the review state of `res_a`, so it gives the
/// refusal of `res_a`.
#[test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
fn a_citation_of_a_resolution_whose_rows_disagree_gives_its_refusal() {
    let (_directory, repo) = repository(Merge::Driver);
    create_resolution(&repo, "res_a");
    super::review_cycle::write(
        &repo,
        "post",
        "rules",
        &json!({"id":"rule_a", "statement":"The system keeps one copy.",
            "severity":"medium", "status":"review", "requirement_ids":["req_a"],
            "resolution_ids":["res_a"]}),
        None,
    );
    decide_resolution_on_two_branches(&repo, "res_a");

    let resolution = refused(&repo, &["api", "resolutions/res_a"]);
    assert_eq!(resolution["kind"], "invalid_state_file", "{resolution}");
    let rule = read(&repo, &["api", "rules/rule_a"]);
    let etag = rule["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let input = directory_file(&repo, "rule.json", &json!({"status":"active"}).to_string());
    let header = format!("If-Match: {etag}");
    let arguments = [
        "api",
        "rules/rule_a",
        "--method",
        "patch",
        "--input",
        &input,
        "--header",
        &header,
    ];
    assert_eq!(refused(&repo, &arguments), resolution);
    read(&repo, &["api", "rules/rule_a"]);
}
