//! Creating a record and a content change to it open its review submission
//! in the same write, for each native record kind.

use provenance_macros::verifies;
use serde_json::Value;

mod support {
    pub mod record_review;
    pub mod review_paths;
}
use support::record_review::{
    accept_current, create_resolution, create_rule, current_submission, data, decide,
    guarded_update, read,
};
use support::review_paths::{create_requirement, init, json_output, provenance};

/// The change opened one pending submission bound to the new revision, and
/// the lifecycle status stayed `status`.
fn assert_pending_at_current_revision(after: &Value, status: &str) {
    assert_eq!(after["status"], status);
    let pending = &after["decision"]["pending"];
    assert!(!pending.is_null(), "no pending submission: {after}");
    assert_eq!(pending["revision"], after["edit"]["revision"]);
    assert!(after["decision"]["current_acceptance"].is_null());
}

fn accepted_active_rule(repo: &str) {
    create_requirement(repo, "req_a");
    create_rule(repo, "rule_a", "req_a");
    accept_current(repo, "rules", "rule", "rule_a");
    guarded_update(repo, "rules", "rule_a", &["--status", "active"]);
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn statement_change_to_an_accepted_rule_opens_a_pending_submission() {
    let (_directory, repo) = init();
    accepted_active_rule(&repo);

    guarded_update(
        &repo,
        "rules",
        "rule_a",
        &["--statement", "The system keeps each stored record."],
    );

    let after = read(&repo, "rules", "rule_a");
    assert_eq!(after["statement"], "The system keeps each stored record.");
    assert_pending_at_current_revision(&after, "active");
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn severity_and_citation_changes_to_an_accepted_rule_open_a_pending_submission() {
    let (_directory, repo) = init();
    accepted_active_rule(&repo);
    guarded_update(&repo, "rules", "rule_a", &["--severity", "low"]);
    assert_pending_at_current_revision(&read(&repo, "rules", "rule_a"), "active");

    let (_directory, repo) = init();
    accepted_active_rule(&repo);
    create_requirement(&repo, "req_b");
    guarded_update(
        &repo,
        "rules",
        "rule_a",
        &["--requirement-ids-json", r#"{"add":["req_b"],"remove":[]}"#],
    );
    let after = read(&repo, "rules", "rule_a");
    assert_eq!(
        after["requirement_ids"],
        serde_json::json!(["req_a", "req_b"])
    );
    assert_pending_at_current_revision(&after, "active");
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn statement_change_after_a_rejection_opens_a_pending_submission() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    create_rule(&repo, "rule_a", "req_a");
    let proposal = current_submission(&repo, "rules", "rule_a");
    decide(&repo, "rules", "rule", "rule_a", &proposal, false);
    let rejected = read(&repo, "rules", "rule_a");
    assert!(rejected["decision"]["pending"].is_null());

    guarded_update(
        &repo,
        "rules",
        "rule_a",
        &["--statement", "The system keeps each record for one year."],
    );

    let after = read(&repo, "rules", "rule_a");
    assert_pending_at_current_revision(&after, rejected["status"].as_str().unwrap());
    assert_ne!(
        after["decision"]["pending"]["proposal_id"],
        proposal.as_str()
    );
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn content_change_back_to_the_accepted_content_opens_no_submission() {
    let (_directory, repo) = init();
    accepted_active_rule(&repo);
    guarded_update(
        &repo,
        "rules",
        "rule_a",
        &["--statement", "The system keeps each stored record."],
    );
    assert!(!read(&repo, "rules", "rule_a")["decision"]["pending"].is_null());

    guarded_update(
        &repo,
        "rules",
        "rule_a",
        &["--statement", "The system keeps each record."],
    );

    let after = read(&repo, "rules", "rule_a");
    assert!(after["decision"]["pending"].is_null(), "{after}");
    assert!(!after["decision"]["current_acceptance"].is_null());
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn position_change_to_an_accepted_resolution_opens_a_pending_submission() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    create_resolution(&repo, "res_a", "req_a");
    accept_current(&repo, "resolutions", "resolution", "res_a");
    let status = read(&repo, "resolutions", "res_a")["status"].clone();

    guarded_update(
        &repo,
        "resolutions",
        "res_a",
        &["--position", "The system stores records in two files."],
    );

    let after = read(&repo, "resolutions", "res_a");
    assert_eq!(after["position"], "The system stores records in two files.");
    assert_pending_at_current_revision(&after, status.as_str().unwrap());
}

#[test]
#[verifies("rule_content_change_opens_review_submission", examples)]
fn creating_a_rule_or_resolution_in_review_opens_its_pending_submission() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    data(
        &repo,
        &[
            "rules",
            "create",
            "--id",
            "rule_a",
            "--status",
            "review",
            "--statement",
            "The system keeps each record.",
            "--requirement-id",
            "req_a",
        ],
    );
    data(
        &repo,
        &[
            "resolutions",
            "create",
            "--id",
            "res_a",
            "--status",
            "review",
            "--title",
            "Store records",
            "--position",
            "The system stores records in one file.",
            "--rationale",
            "One file is simple.",
            "--requirement-id",
            "req_a",
        ],
    );

    for (collection, id) in [("rules", "rule_a"), ("resolutions", "res_a")] {
        let created = read(&repo, collection, id);
        assert_pending_at_current_revision(&created, "review");
        let review = json_output(provenance().args([
            id, "get", "--view", "review", "--repo", &repo, "--format", "json",
        ]));
        assert_eq!(
            review["review"]["decision"]["pending"]["proposal_id"],
            created["decision"]["pending"]["proposal_id"]
        );
    }
}
