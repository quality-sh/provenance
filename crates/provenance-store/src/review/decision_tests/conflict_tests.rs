use super::*;
use crate::write_error::{WriteError, WriteFailure};

#[test]
fn repeated_decision_reports_the_current_review_identity() {
    let (_temp, store, revision) = enrolled();
    decide(
        &store,
        "decide-1",
        "prop-1",
        "disp-1",
        "rejected",
        &reviewer("reviewer"),
        &json!({}),
    )
    .unwrap();

    let error = decide(
        &store,
        "decide-2",
        "prop-1",
        "disp-2",
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
fn stale_decision_reports_the_pending_submission_and_current_revision() {
    let (_temp, store, _) = enrolled();
    let current_revision = edit(&store, "edit-2", "Statement v2");

    let error = decide(
        &store,
        "decide-1",
        "prop-1",
        "disp-1",
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap_err();

    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: Some(submission),
            current_revision: revision,
        } if submission.as_str() == "prop-1" && revision == current_revision
    ));
}

#[test]
fn withdrawn_decision_reports_the_current_review_identity() {
    let (_temp, store, revision) = enrolled();
    withdraw(&store, "withdraw-1", "prop-1").unwrap();

    let error = decide(
        &store,
        "decide-1",
        "prop-1",
        "disp-1",
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
fn stale_and_terminal_withdrawals_are_review_conflicts() {
    let (_temp, store, _) = enrolled();
    let current_revision = edit(&store, "edit-2", "Statement v2");
    let stale = WriteError(withdraw(&store, "withdraw-1", "prop-1").unwrap_err());
    assert!(matches!(
        stale.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: Some(submission),
            current_revision: revision,
        } if submission.as_str() == "prop-1" && revision == current_revision
    ));

    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let revision = edit(&store, "edit-1", "Statement v1");
    submit(&store, "submit-1", "prop-1", None, None).unwrap();
    withdraw(&store, "withdraw-1", "prop-1").unwrap();
    let repeated = WriteError(withdraw(&store, "withdraw-2", "prop-1").unwrap_err());
    assert!(matches!(
        repeated.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: None,
            current_revision,
        } if current_revision == revision
    ));
}
