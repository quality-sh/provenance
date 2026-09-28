use super::*;
use crate::write_error::{WriteError, WriteFailure};

#[test]
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
fn stale_decision_reports_the_pending_submission_and_current_revision() {
    let (_temp, store, _, proposal) = enrolled();
    let current_revision = edit(&store, "edit-2", "Statement v2");

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
            current_revision: revision,
        } if revision == current_revision
    ));
}

#[test]
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
fn stale_and_terminal_withdrawals_are_review_conflicts() {
    let (_temp, store, _, proposal) = enrolled();
    let current_revision = edit(&store, "edit-2", "Statement v2");
    let stale = WriteError(withdraw(&store, &proposal).unwrap_err());
    assert!(matches!(
        stale.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: None,
            current_revision: revision,
        } if revision == current_revision
    ));

    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let revision = edit(&store, "edit-1", "Statement v1");
    let proposal = submit(&store, None, None).unwrap().proposal_id;
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
fn superseded_submission_allows_a_new_review_cycle() {
    let (_temp, store, _, proposal_1) = enrolled();
    let revision_2 = edit(&store, "edit-2", "Statement v2");

    for error in [
        decide(
            &store,
            &proposal_1,
            "accepted",
            &reviewer("reviewer"),
            &artifact(),
        )
        .unwrap_err(),
        withdraw(&store, &proposal_1).unwrap_err(),
    ] {
        assert!(matches!(
            WriteError(error).safe(),
            WriteFailure::ReviewSubmissionConflict {
                current_submission: None,
                current_revision,
            } if current_revision == revision_2
        ));
    }

    let proposal_2 = submit(&store, None, Some(revision_2.as_str()))
        .unwrap()
        .proposal_id;
    assert_ne!(proposal_1, proposal_2);
    decide(
        &store,
        &proposal_2,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap();
}

#[test]
fn stale_and_repeated_submissions_are_typed_conflicts() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    let revision_1 = edit(&store, "edit-1", "Statement v1");
    let revision_2 = edit(&store, "edit-2", "Statement v2");
    let stale = WriteError(submit(&store, None, Some(revision_1.as_str())).unwrap_err());
    assert_eq!(stale.status(), 409);
    assert!(matches!(
        stale.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: None,
            current_revision,
        } if current_revision == revision_2
    ));

    let submission = submit(&store, None, Some(revision_2.as_str())).unwrap();
    let repeated = WriteError(submit(&store, None, Some(revision_2.as_str())).unwrap_err());
    assert_eq!(repeated.status(), 409);
    assert!(matches!(
        repeated.safe(),
        WriteFailure::ReviewSubmissionConflict {
            current_submission: Some(current),
            current_revision,
        } if current == submission.proposal_id && current_revision == revision_2
    ));
    assert_eq!(store.list_proposal_definitions(&scope()).unwrap().len(), 1);
}

#[test]
fn server_creates_unique_identities_across_review_cycles() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    edit(&store, "edit-1", "Statement v1");
    let submission_1 = submit(&store, None, None).unwrap();
    let decision_1 = decide(
        &store,
        &submission_1.proposal_id,
        "rejected",
        &reviewer("reviewer"),
        &json!({}),
    )
    .unwrap();
    edit(&store, "edit-2", "Statement v2");
    let submission_2 = submit(&store, Some(&submission_1.proposal_id), None).unwrap();
    let decision_2 = decide(
        &store,
        &submission_2.proposal_id,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap();

    assert_ne!(submission_1.proposal_id, submission_2.proposal_id);
    let request_ids = [
        &submission_1.request_id,
        &decision_1.request_id,
        &submission_2.request_id,
        &decision_2.request_id,
    ];
    assert_eq!(
        request_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        request_ids.len()
    );
    assert_ne!(decision_1.disposition_id, decision_2.disposition_id);
}
