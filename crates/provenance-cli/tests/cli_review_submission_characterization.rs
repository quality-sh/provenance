//! Characterization tests: the review submission behaviour of Requirement
//! saves and of lifecycle-only changes. These pin the base behaviour that the
//! content-change submission fix for other record kinds must keep. They are
//! implementation aids and verify no Rule.

mod support {
    pub mod record_review;
    pub mod review_paths;
}
use support::record_review::{
    accept_current, create_resolution, create_rule, decide, guarded_update, read,
};
use support::review_paths::{create_requirement, init};

fn pending_revision(record: &serde_json::Value) -> Option<String> {
    record["decision"]["pending"]["revision"]
        .as_str()
        .map(str::to_owned)
}

/// Accepts the automatic submission that Requirement creation opens.
fn accept_requirement(repo: &str, id: &str) {
    let created = read(repo, "requirements", id);
    let proposal = created["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    decide(repo, "requirements", "requirement", id, &proposal, true);
}

#[test]
fn requirement_content_change_after_acceptance_opens_a_pending_submission() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    accept_requirement(&repo, "req_a");
    assert!(read(&repo, "requirements", "req_a")["decision"]["pending"].is_null());

    guarded_update(
        &repo,
        "requirements",
        "req_a",
        &["--statement", "The system keeps each review result."],
    );

    let after = read(&repo, "requirements", "req_a");
    assert_eq!(after["status"], "active");
    assert_eq!(
        pending_revision(&after).as_deref(),
        after["edit"]["revision"].as_str()
    );
    assert!(after["decision"]["current_acceptance"].is_null());
}

#[test]
fn requirement_lifecycle_change_opens_no_submission() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    accept_requirement(&repo, "req_a");
    let before = read(&repo, "requirements", "req_a");

    guarded_update(&repo, "requirements", "req_a", &["--status", "refinement"]);

    let after = read(&repo, "requirements", "req_a");
    assert_eq!(after["status"], "refinement");
    assert!(after["decision"]["pending"].is_null());
    assert_eq!(after["edit"]["revision"], before["edit"]["revision"]);
    assert_eq!(
        after["decision"]["current_acceptance"],
        before["decision"]["current_acceptance"]
    );
}

#[test]
fn rule_lifecycle_change_after_acceptance_opens_no_submission() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    create_rule(&repo, "rule_a", "req_a");
    accept_current(&repo, "rules", "rule", "rule_a");
    let before = read(&repo, "rules", "rule_a");
    assert!(!before["decision"]["current_acceptance"].is_null());

    guarded_update(&repo, "rules", "rule_a", &["--status", "active"]);

    let after = read(&repo, "rules", "rule_a");
    assert_eq!(after["status"], "active");
    assert!(after["decision"]["pending"].is_null());
    assert_eq!(
        after["decision"]["current_acceptance"],
        before["decision"]["current_acceptance"]
    );
}

#[test]
fn resolution_lifecycle_change_after_acceptance_opens_no_submission() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    create_resolution(&repo, "res_a", "req_a");
    accept_current(&repo, "resolutions", "resolution", "res_a");
    let before = read(&repo, "resolutions", "res_a");

    guarded_update(
        &repo,
        "resolutions",
        "res_a",
        &["--review-on", "2027-01-01"],
    );

    let after = read(&repo, "resolutions", "res_a");
    assert_eq!(after["review_on"], "2027-01-01");
    assert!(after["decision"]["pending"].is_null());
    assert_eq!(
        after["decision"]["current_acceptance"],
        before["decision"]["current_acceptance"]
    );
}

#[test]
fn content_change_to_a_never_submitted_rule_opens_no_submission() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_a");
    create_rule(&repo, "rule_a", "req_a");

    guarded_update(
        &repo,
        "rules",
        "rule_a",
        &["--statement", "The system keeps each stored record."],
    );

    let after = read(&repo, "rules", "rule_a");
    assert_eq!(after["statement"], "The system keeps each stored record.");
    assert!(after["decision"]["pending"].is_null());
    assert_eq!(after["decision"]["decisions"], serde_json::json!([]));
}
