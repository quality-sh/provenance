//! The refusal of a scope import or a scope export of a scope that holds review state that a
//! scope export file cannot hold.

use crate::cli_ste_statement_paths::support::provenance_tree;
use crate::support::import_reviews::{
    document, export, import, import_with, patch_statement, refusal, repository,
    restated_repository, reviews, verify, AFTER, BEFORE,
};
use provenance_macros::verifies;

const REQUIREMENT_REVIEW_REFUSAL: &str = "Error: scope `default` holds a Requirement review \
     record in scopes/default/requirements/review.jsonl, which a scope export file cannot hold, \
     so an import into this scope and an export of it are not possible; read and change the \
     Requirements, Rules and other records of this scope with the routes that `provenance api \
     --scope default` lists\n";

const REVIEW_HISTORY_REFUSAL: &str = "Error: scope `default` holds a review submission, a \
     Discussion or a withdrawal, which a scope export file cannot hold, so an import into this \
     scope and an export of it are not possible; read and change the Requirements, Rules and \
     other records of this scope with the routes that `provenance api --scope default` lists\n";

#[test]
#[verifies("rule_import_export_refuse_requirement_reviews", examples)]
fn an_import_on_a_scope_with_a_requirement_review_record_is_refused_and_changes_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = restated_repository(dir.path());
    let before = provenance_tree(&repo);

    let refused = refusal(import(&repo, &document(BEFORE, &["rule_a", "rule_b"])));

    assert_eq!(refused, (Some(1), REQUIREMENT_REVIEW_REFUSAL.to_owned()));
    assert_eq!(provenance_tree(&repo), before);
}

#[test]
#[verifies("rule_import_export_refuse_requirement_reviews", examples)]
fn a_dry_run_of_an_import_on_a_scope_with_a_requirement_review_record_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let repo = restated_repository(dir.path());
    let before = provenance_tree(&repo);

    let refused = refusal(import_with(
        &repo,
        &document(BEFORE, &["rule_a", "rule_b"]),
        &["--dry-run"],
    ));

    assert_eq!(refused, (Some(1), REQUIREMENT_REVIEW_REFUSAL.to_owned()));
    assert_eq!(provenance_tree(&repo), before);
}

#[test]
#[verifies("rule_import_export_refuse_requirement_reviews", examples)]
fn an_export_of_a_scope_with_a_requirement_review_record_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let repo = restated_repository(dir.path());

    let refused = refusal(export(&repo));

    assert_eq!(refused, (Some(1), REQUIREMENT_REVIEW_REFUSAL.to_owned()));
}

#[test]
#[verifies("rule_import_export_refuse_requirement_reviews", examples)]
fn an_import_on_a_scope_whose_requirement_review_records_a_verification_run_cleared_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let repo = restated_repository(dir.path());
    verify(&repo, "rule_a");
    verify(&repo, "rule_b");
    let records = reviews(&repo);
    assert_eq!(records.len(), 2);
    assert!(
        records.iter().all(|record| record["cleared_at"].is_i64()),
        "{records:?}"
    );

    let refused = refusal(import(&repo, &document(BEFORE, &["rule_a", "rule_b"])));

    assert_eq!(refused, (Some(1), REQUIREMENT_REVIEW_REFUSAL.to_owned()));
}

/// A change through the API writes a Requirement review record and opens a review submission.
/// The submission is the cause that the refusal names.
#[test]
#[verifies("rule_import_export_refuse_requirement_reviews", examples)]
fn an_export_of_a_scope_with_a_submission_and_a_review_record_names_the_submission() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &["rule_a"])).success();
    patch_statement(&repo, AFTER);
    assert_eq!(reviews(&repo).len(), 1);

    let refused = refusal(export(&repo));

    assert_eq!(refused, (Some(1), REVIEW_HISTORY_REFUSAL.to_owned()));
}

#[test]
#[verifies("rule_import_export_refuse_requirement_reviews", examples)]
fn an_import_on_a_scope_without_review_state_passes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &["rule_a"])).success();

    import(&repo, &document(BEFORE, &["rule_a", "rule_b"])).success();

    export(&repo).success();
}
