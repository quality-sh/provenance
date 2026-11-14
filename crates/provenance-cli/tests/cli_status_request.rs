//! A change of a record status to active or archived waits for a reviewer.
//! The CLI write opens a status request, and an approval sets the status.

use provenance_macros::verifies;

use crate::support;
use support::record_review::{
    accept_current, create_resolution, create_rule, data, decide, guarded_update,
    pending_status_request, read, withdraw,
};
use support::review_paths::{create_requirement, init, provenance};

const ARCHIVE_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// A repository with one draft Rule whose creation a reviewer accepted.
fn accepted_draft_rule() -> (tempfile::TempDir, String) {
    let (directory, repo) = init();
    create_requirement(&repo, "req_a");
    create_rule(&repo, "rule_a", "req_a");
    accept_current(&repo, "rules", "rule", "rule_a");
    (directory, repo)
}

#[test]
#[verifies("rule_status_change_waits_for_review", examples)]
#[verifies("rule_status_approval_sets_status", examples)]
fn rule_activation_waits_for_the_reviewer() {
    let (_directory, repo) = accepted_draft_rule();

    guarded_update(&repo, "rules", "rule_a", &["--status", "active"]);

    let after = read(&repo, "rules", "rule_a");
    assert_eq!(after["status"], "draft", "{after}");
    let (proposal, request) = pending_status_request(&repo, "rules", "rule_a");
    assert_eq!(request["status"], "active");
    assert_eq!(
        after["decision"]["pending"]["status_request"], request,
        "{after}"
    );

    decide(&repo, "rules", "rule", "rule_a", &proposal, true);

    let accepted = read(&repo, "rules", "rule_a");
    assert_eq!(accepted["status"], "active", "{accepted}");
    assert!(accepted["decision"]["pending"].is_null());
}

#[test]
#[verifies("rule_status_change_waits_for_review", examples)]
#[verifies("rule_status_approval_sets_status", examples)]
fn rule_archive_waits_for_the_reviewer() {
    let (_directory, repo) = accepted_draft_rule();

    guarded_update(
        &repo,
        "rules",
        "rule_a",
        &[
            "--status",
            "archived",
            "--archived-in-commit",
            ARCHIVE_COMMIT,
        ],
    );

    let after = read(&repo, "rules", "rule_a");
    assert_eq!(after["status"], "draft", "{after}");
    assert!(after.get("archived_in_commit").is_none(), "{after}");
    let (proposal, request) = pending_status_request(&repo, "rules", "rule_a");
    assert_eq!(request["status"], "archived");
    assert_eq!(request["archived_in_commit"]["commit"], ARCHIVE_COMMIT);

    decide(&repo, "rules", "rule", "rule_a", &proposal, true);

    let archived = read(&repo, "rules", "rule_a");
    assert_eq!(archived["status"], "archived", "{archived}");
    assert_eq!(archived["archived_in_commit"]["commit"], ARCHIVE_COMMIT);
}

#[test]
#[verifies("rule_status_refusal_keeps_status", examples)]
fn rejection_and_withdrawal_keep_the_rule_status() {
    let (_directory, repo) = accepted_draft_rule();

    guarded_update(&repo, "rules", "rule_a", &["--status", "active"]);
    let (rejected, _) = pending_status_request(&repo, "rules", "rule_a");
    decide(&repo, "rules", "rule", "rule_a", &rejected, false);
    assert_eq!(read(&repo, "rules", "rule_a")["status"], "draft");

    guarded_update(&repo, "rules", "rule_a", &["--status", "active"]);
    let (withdrawn, _) = pending_status_request(&repo, "rules", "rule_a");
    withdraw(&repo, "rules", "rule_a", &withdrawn);

    let after = read(&repo, "rules", "rule_a");
    assert_eq!(after["status"], "draft", "{after}");
    assert!(after["decision"]["pending"].is_null(), "{after}");
}

#[test]
#[verifies("rule_status_request_changes_one_field", examples)]
fn a_status_request_with_another_field_is_refused() {
    let (_directory, repo) = accepted_draft_rule();
    let before = read(&repo, "rules", "rule_a");

    let output = provenance()
        .args([
            "rule_a",
            "update",
            "--status",
            "active",
            "--severity",
            "low",
            "--repo",
            &repo,
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        format!("{stdout}{stderr}").contains("cannot change other fields"),
        "{stdout}{stderr}"
    );
    let after = read(&repo, "rules", "rule_a");
    assert_eq!(after["status"], before["status"]);
    assert_eq!(after["severity"], before["severity"]);
    assert!(after["decision"]["pending"].is_null(), "{after}");
}

#[test]
#[verifies("rule_new_active_rule_waits_for_review", examples)]
fn a_new_active_rule_waits_for_the_reviewer() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");

    data(
        &repo,
        &[
            "rule_a",
            "create",
            "--type",
            "rule",
            "--status",
            "active",
            "--statement",
            "The system keeps each record.",
            "--requirement-id",
            "req_a",
        ],
    );

    let created = read(&repo, "rules", "rule_a");
    assert_eq!(created["status"], "review", "{created}");
    let (proposal, request) = pending_status_request(&repo, "rules", "rule_a");
    assert_eq!(request["status"], "active");

    decide(&repo, "rules", "rule", "rule_a", &proposal, true);

    assert_eq!(read(&repo, "rules", "rule_a")["status"], "active");
}

#[test]
fn a_new_archived_rule_is_refused() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");

    provenance()
        .args([
            "rule_a",
            "create",
            "--type",
            "rule",
            "--status",
            "archived",
            "--archived-in-commit",
            ARCHIVE_COMMIT,
            "--statement",
            "The system keeps each record.",
            "--requirement-id",
            "req_a",
            "--repo",
            &repo,
        ])
        .assert()
        .failure();
}

/// Runs a JSON command that must fail and returns its standard output and
/// standard error.
fn refused(repo: &str, args: &[&str]) -> String {
    let mut all = args.to_vec();
    all.extend(["--repo", repo, "--format", "json"]);
    let output = provenance().args(all).output().unwrap();
    assert!(!output.status.success());
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A repository with the proposed Resolution `res_a`.
fn proposed_resolution() -> (tempfile::TempDir, String) {
    let (directory, repo) = init();
    create_requirement(&repo, "req_a");
    create_resolution(&repo, "res_a", "req_a");
    (directory, repo)
}

const RULE_CITING_RES_A: [&str; 9] = [
    "rule_a",
    "create",
    "--type",
    "rule",
    "--statement",
    "The system keeps each record.",
    "--requirement-id",
    "req_a",
    "--resolution-id",
];

#[test]
#[verifies("rule_status_request_refused_like_direct_write", examples)]
fn a_new_active_rule_citing_a_proposed_resolution_is_refused() {
    let (_directory, repo) = proposed_resolution();

    let mut args = RULE_CITING_RES_A.to_vec();
    args.extend(["res_a", "--status", "active"]);
    let output = refused(&repo, &args);

    assert!(output.contains("resolution_not_approved"), "{output}");
    let rules = data(&repo, &["api", "rules"]);
    assert!(!rules.to_string().contains("rule_a"), "{rules}");
}

#[test]
#[verifies("rule_status_request_refused_like_direct_write", examples)]
fn the_direct_write_refusal_comes_before_the_pending_check() {
    let (_directory, repo) = proposed_resolution();
    let mut args = RULE_CITING_RES_A.to_vec();
    args.push("res_a");
    data(&repo, &args);
    let before = read(&repo, "rules", "rule_a");
    let etag = before["edit"]["etag"].as_str().unwrap().to_owned();

    let output = refused(
        &repo,
        &[
            "rule_a",
            "update",
            "--if-match",
            &etag,
            "--status",
            "active",
        ],
    );

    assert!(output.contains("resolution_not_approved"), "{output}");
    assert!(!output.contains("review_submission_conflict"), "{output}");
    let after = read(&repo, "rules", "rule_a");
    assert_eq!(after["status"], "draft", "{after}");
    assert_eq!(
        after["decision"]["pending"], before["decision"]["pending"],
        "{after}"
    );
    assert!(after["decision"]["pending"]["status_request"].is_null());
}
