use super::*;
use crate::write_error::{WriteError, WriteFailure};

#[test]
#[provenance_macros::verifies("rule_review_conflict_returns_current_value", examples)]
fn repeated_decision_reports_the_current_review_identity() {
    let (_temp, store, revision, proposal) = enrolled();
    decide(
        &store,
        &proposal,
        "rejected",
        &reviewer("reviewer"),
        &json!({}),
    )
    .unwrap();

    let error = decide(
        &store,
        &proposal,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap_err();

    let error = WriteError(error);
    assert_eq!(error.status(), 409);
    assert!(matches!(
        error.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: None,
            current_revision,
        } if current_revision == revision
    ));
}

#[test]
#[provenance_macros::verifies("rule_review_conflict_returns_current_value", examples)]
fn stale_decision_reports_the_pending_submission_and_current_revision() {
    let (_temp, store, _, proposal) = enrolled();
    let current_revision = edit(&store, "edit-2", "Revised statement");

    let error = decide(
        &store,
        &proposal,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap_err();

    let current_proposal = state(&store).pending.unwrap().proposal_id;
    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: Some(submission),
            current_revision: revision,
        } if submission == current_proposal && revision == current_revision
    ));
}

#[test]
#[provenance_macros::verifies("rule_review_conflict_not_merged", examples)]
fn stale_decision_does_not_replace_the_pending_submission() {
    let (_temp, store, _, proposal) = enrolled();
    edit(&store, "edit-2", "Revised statement");
    let current_proposal = state(&store).pending.unwrap().proposal_id;

    decide(
        &store,
        &proposal,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap_err();

    assert_eq!(state(&store).pending.unwrap().proposal_id, current_proposal);
    assert_eq!(
        store.list_dispositions(&scope()).unwrap(),
        [] as [provenance_core::DispositionRecord; 0]
    );
    assert_eq!(
        store.list_requirements(&scope()).unwrap()[0].statement,
        "Revised statement"
    );
}

#[test]
#[provenance_macros::verifies("rule_review_conflict_returns_current_value", examples)]
fn withdrawn_decision_reports_the_current_review_identity() {
    let (_temp, store, revision, proposal) = enrolled();
    withdraw(&store, &proposal).unwrap();

    let error = decide(
        &store,
        &proposal,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap_err();

    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: None,
            current_revision,
        } if current_revision == revision
    ));
}

#[test]
#[provenance_macros::verifies("rule_review_conflict_returns_current_value", examples)]
fn stale_and_terminal_withdrawals_are_review_conflicts() {
    let (_temp, store, _, proposal) = enrolled();
    let current_revision = edit(&store, "edit-2", "Revised statement");
    let stale = WriteError(withdraw(&store, &proposal).unwrap_err());
    let current_proposal = state(&store).pending.unwrap().proposal_id;
    assert!(matches!(
        stale.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: Some(submission),
            current_revision: revision,
        } if submission == current_proposal && revision == current_revision
    ));

    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let revision = edit(&store, "edit-1", "Statement v1");
    let proposal = automatic_submission(&store).proposal_id;
    withdraw(&store, &proposal).unwrap();
    let repeated = WriteError(withdraw(&store, &proposal).unwrap_err());
    assert!(matches!(
        repeated.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: None,
            current_revision,
        } if current_revision == revision
    ));
}

#[test]
#[provenance_macros::verifies("rule_review_conflict_returns_current_value", examples)]
fn superseded_submission_reports_the_current_review_identity() {
    let (_temp, store, _, proposal_1) = enrolled();
    let revision_2 = edit(&store, "edit-2", "Revised statement");

    let proposal_2 = state(&store).pending.unwrap().proposal_id;
    let error = decide(
        &store,
        &proposal_1,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap_err();
    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: Some(submission),
            current_revision,
        } if submission == proposal_2 && current_revision == revision_2
    ));
}

#[test]
#[provenance_macros::verifies("rule_review_conflict_returns_current_value", examples)]
fn stale_and_repeated_submissions_are_typed_conflicts() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let revision_1 = edit(&store, "edit-1", "Statement v1");
    let revision_2 = edit(&store, "edit-2", "Revised statement");
    let stale = WriteError(submit(&store, None, Some(revision_1.as_str())).unwrap_err());
    assert_eq!(stale.status(), 409);
    let submission = automatic_submission(&store);
    assert!(matches!(
        stale.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: Some(current),
            current_revision,
        } if current == submission.proposal_id && current_revision == revision_2
    ));

    let repeated = WriteError(submit(&store, None, Some(revision_2.as_str())).unwrap_err());
    assert_eq!(repeated.status(), 409);
    assert!(matches!(
        repeated.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: Some(current),
            current_revision,
        } if current == submission.proposal_id && current_revision == revision_2
    ));
    assert_eq!(store.list_proposal_definitions(&scope()).unwrap().len(), 2);
}
