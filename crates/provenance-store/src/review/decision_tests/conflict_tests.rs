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
fn superseded_submission_allows_a_new_review_cycle() {
    let (_temp, store, _, proposal_1) = enrolled();
    let revision_2 = edit(&store, "edit-2", "Revised statement");

    let proposal_2 = state(&store).pending.unwrap().proposal_id;
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
                current_submission: Some(submission),
                current_revision,
            } if submission == proposal_2 && current_revision == revision_2
        ));
    }

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

#[test]
#[provenance_macros::verifies("rule_review_request_identity_server_created", examples)]
#[provenance_macros::verifies("rule_review_proposal_identity_server_created", examples)]
#[provenance_macros::verifies("rule_review_disposition_identity_server_created", examples)]
fn server_creates_unique_identities_across_review_cycles() {
    let temp = fixture();
    let store = open(Utf8Path::from_path(temp.path()).unwrap());
    edit(&store, "edit-1", "Statement v1");
    let submission_1 = automatic_submission(&store);
    let withdrawal = withdraw(&store, &submission_1.proposal_id).unwrap();
    let explicit_submission = submit(&store, None, None).unwrap();
    let rejection = decide(
        &store,
        &explicit_submission.proposal_id,
        "rejected",
        &reviewer("reviewer"),
        &json!({}),
    )
    .unwrap();
    edit(&store, "edit-2", "Revised statement");
    let submission_2 = automatic_submission(&store);
    let acceptance = decide(
        &store,
        &submission_2.proposal_id,
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap();

    assert_ne!(submission_1.proposal_id, explicit_submission.proposal_id);
    assert_ne!(explicit_submission.proposal_id, submission_2.proposal_id);
    let request_ids = [
        &submission_1.request_id,
        &withdrawal.request_id,
        &explicit_submission.request_id,
        &rejection.request_id,
        &submission_2.request_id,
        &acceptance.request_id,
    ];
    assert_eq!(
        request_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        request_ids.len()
    );
    assert_ne!(rejection.disposition_id, acceptance.disposition_id);

    let submit_with_id = json!({
        "scope_id":"default","actor":"agent","record_kind":"requirement",
        "record_id":"req_a","title":"Title","summary":"Summary","source_ids":[],
        "evidence_references":[],"builds_on":[],"request_id":"caller-request",
        "proposal_id":"caller-proposal"
    });
    assert!(serde_json::from_value::<crate::review::SubmitRecordReview>(submit_with_id).is_err());
    let decision_with_id = json!({
        "scope_id":"default","actor":reviewer("reviewer"),
        "proposal_id":submission_2.proposal_id,"decision":"accepted",
        "canonical_artifact":artifact()["canonical_artifact"],
        "request_id":"caller-request","disposition_id":"caller-disposition"
    });
    assert!(serde_json::from_value::<crate::review::DecideRecordReview>(decision_with_id).is_err());
    let withdrawal_with_id = json!({
        "scope_id":"default","actor":"agent","proposal_id":submission_2.proposal_id,
        "request_id":"caller-request"
    });
    assert!(
        serde_json::from_value::<crate::review::WithdrawRecordReview>(withdrawal_with_id).is_err()
    );
}
