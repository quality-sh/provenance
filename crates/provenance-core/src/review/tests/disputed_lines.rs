//! Each line of a review row takes part in each check of the record that it
//! names. A row ID on lines that disagree is one more fault and does not
//! hide its lines from the other checks, so one reading gives each fault.

use super::rows::{
    changed_submission, decision, decision_at, id, submission, submission_at, withdrawal,
    withdrawal_at, REQ_A,
};
use crate::review::{read_review_rows, ReviewRowFault, RowAt, StoredReviewRows, SubmissionLines};
use crate::{DispositionDecision, DispositionRecord, NodeType, ProposalCard, Withdrawal};
use provenance_macros::verifies;

fn faults(
    proposals: &[ProposalCard],
    dispositions: &[DispositionRecord],
    withdrawals: &[Withdrawal],
) -> Vec<ReviewRowFault> {
    read_review_rows(StoredReviewRows {
        proposals,
        dispositions,
        withdrawals,
    })
    .faults()
    .cloned()
    .collect()
}

fn lines_of(submission: usize, lines: Vec<RowAt>) -> SubmissionLines {
    SubmissionLines {
        submission: submission_at(submission),
        lines,
    }
}

fn sub_a_on_lines(lines: [usize; 2]) -> ReviewRowFault {
    ReviewRowFault::IdOnTwoLines {
        id: id("sub_a"),
        lines: lines.map(submission_at).to_vec(),
        second: submission_at(lines[1]),
    }
}

/// A hand edit copies the submission `sub_a` of `req_a` with a new ID and
/// changes the summary of a second line of `sub_a`. The three lines have
/// review cycle 1, so one reading gives the fault of the ID and the fault of
/// the shared cycle, which names each line of `sub_a`.
#[test]
#[verifies("rule_check_reports_review_row_faults", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn a_submission_on_lines_that_disagree_takes_part_in_the_cycle_check() {
    let proposals = [
        submission("sub_a", REQ_A, 1),
        submission("sub_copy", REQ_A, 1),
        changed_submission("sub_a", REQ_A, 1),
    ];
    assert_eq!(
        faults(&proposals, &[], &[]),
        [
            sub_a_on_lines([0, 2]),
            ReviewRowFault::OneCycleTwoSubmissions {
                cycle: 1,
                submissions: vec![
                    lines_of(0, vec![submission_at(0), submission_at(2)]),
                    lines_of(1, vec![submission_at(1)]),
                ],
            },
        ]
    );
}

/// The lines of `sub_a` have the review cycles 1 and 2, and a different
/// submission of `req_a` has each cycle. Each cycle of a line gives its
/// fault, so the line that the person keeps cannot show a new fault.
#[test]
#[verifies("rule_check_reports_review_row_faults", examples)]
fn each_cycle_of_a_submission_on_lines_that_disagree_is_checked() {
    let proposals = [
        submission("sub_a", REQ_A, 1),
        submission("sub_b", REQ_A, 1),
        changed_submission("sub_a", REQ_A, 2),
        submission("sub_c", REQ_A, 2),
    ];
    let sub_a = vec![submission_at(0), submission_at(2)];
    assert_eq!(
        faults(&proposals, &[], &[]),
        [
            sub_a_on_lines([0, 2]),
            ReviewRowFault::OneCycleTwoSubmissions {
                cycle: 1,
                submissions: vec![
                    lines_of(0, sub_a.clone()),
                    lines_of(1, vec![submission_at(1)]),
                ],
            },
            ReviewRowFault::OneCycleTwoSubmissions {
                cycle: 2,
                submissions: vec![lines_of(2, sub_a), lines_of(3, vec![submission_at(3)])],
            },
        ]
    );
}

/// The decisions and the withdrawals of a submission whose ID is on lines
/// that disagree give their faults beside the fault of the ID.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
fn the_rows_of_a_submission_on_lines_that_disagree_give_their_faults() {
    let proposals = [
        submission("sub_a", REQ_A, 1),
        changed_submission("sub_a", REQ_A, 1),
    ];
    let approved = [decision("dec_a", "sub_a", "accepted", "reviewer")];
    let withdrawn = [withdrawal("wd_a", "sub_a", "agent")];
    assert_eq!(
        faults(&proposals, &approved, &withdrawn),
        [
            sub_a_on_lines([0, 1]),
            ReviewRowFault::AcceptedAndWithdrawn {
                submission: id("sub_a"),
                approvals: vec![decision_at(0)],
                withdrawals: vec![withdrawal_at(0)],
            },
        ]
    );
    let two_results = [
        decision("dec_a", "sub_a", "accepted", "reviewer"),
        decision("dec_b", "sub_a", "rejected", "reviewer"),
    ];
    assert_eq!(
        faults(&proposals, &two_results, &[]),
        [
            sub_a_on_lines([0, 1]),
            ReviewRowFault::DifferentResults {
                submission: id("sub_a"),
                decisions: vec![decision_at(0), decision_at(1)],
                second: decision_at(1),
            },
        ]
    );
}

/// A decision ID and a withdrawal ID on lines that disagree: each line
/// takes part in the result check of its submission, and a withdrawal line
/// that names no submission gives the fault of the scope.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
fn the_lines_of_a_decision_or_withdrawal_that_disagree_are_checked() {
    let proposals = [submission("sub_a", REQ_A, 1)];
    let dispositions = [
        decision("dec_a", "sub_a", "accepted", "reviewer"),
        decision("dec_a", "sub_a", "rejected", "reviewer"),
    ];
    let withdrawals = [
        withdrawal("wd_a", "sub_a", "agent"),
        withdrawal("wd_a", "sub_none", "agent"),
    ];
    let decision_on_lines = ReviewRowFault::IdOnTwoLines {
        id: id("dec_a"),
        lines: vec![decision_at(0), decision_at(1)],
        second: decision_at(1),
    };
    let no_submission = ReviewRowFault::NoSubmission {
        proposal: id("sub_none"),
        lines: vec![withdrawal_at(1)],
    };
    let reading = read_review_rows(StoredReviewRows {
        proposals: &proposals,
        dispositions: &dispositions,
        withdrawals: &withdrawals,
    });
    assert_eq!(
        reading.faults().cloned().collect::<Vec<_>>(),
        [
            decision_on_lines,
            ReviewRowFault::IdOnTwoLines {
                id: id("wd_a"),
                lines: vec![withdrawal_at(0), withdrawal_at(1)],
                second: withdrawal_at(1),
            },
            no_submission.clone(),
            ReviewRowFault::DifferentResults {
                submission: id("sub_a"),
                decisions: vec![decision_at(0), decision_at(1)],
                second: decision_at(1),
            },
            ReviewRowFault::AcceptedAndWithdrawn {
                submission: id("sub_a"),
                approvals: vec![decision_at(0)],
                withdrawals: vec![withdrawal_at(0)],
            },
        ]
    );
    assert_eq!(reading.scope_fault(), Some(&no_submission));
}

/// A list read takes the result of the decisions of a submission only when
/// each row that names the submission agrees.
#[test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
#[verifies("rule_same_result_decisions_read_as_one", examples)]
fn a_list_read_takes_the_result_only_from_rows_that_agree() {
    let one = [submission("sub_a", REQ_A, 1)];
    let disputed = [
        submission("sub_a", REQ_A, 1),
        changed_submission("sub_a", REQ_A, 1),
    ];
    let accepted = |id, actor| decision(id, "sub_a", "accepted", actor);
    let withdrawn = [withdrawal("wd_a", "sub_a", "agent")];
    let cases: [(&[ProposalCard], Vec<DispositionRecord>, &[Withdrawal], _); 7] = [
        (&one, vec![], &[], None),
        (
            &one,
            vec![accepted("dec_a", "reviewer"), accepted("dec_b", "lead")],
            &[],
            Some(DispositionDecision::Accepted),
        ),
        (
            &one,
            vec![decision("dec_a", "sub_a", "rejected", "reviewer")],
            &withdrawn,
            Some(DispositionDecision::Rejected),
        ),
        (&one, vec![accepted("dec_a", "reviewer")], &withdrawn, None),
        (
            &one,
            vec![
                accepted("dec_a", "reviewer"),
                decision("dec_b", "sub_a", "rejected", "reviewer"),
            ],
            &[],
            None,
        ),
        (
            &one,
            vec![accepted("dec_a", "reviewer"), accepted("dec_a", "lead")],
            &[],
            None,
        ),
        (&disputed, vec![accepted("dec_a", "reviewer")], &[], None),
    ];
    for (index, (proposals, dispositions, withdrawals, expected)) in cases.iter().enumerate() {
        let reading = read_review_rows(StoredReviewRows {
            proposals,
            dispositions,
            withdrawals,
        });
        assert_eq!(
            reading.agreed_decision(&id("sub_a")),
            *expected,
            "case {index}"
        );
    }
}

/// The record of a submission whose ID is on lines that disagree refuses
/// with the fault of the ID, and the record of a different line reads.
#[test]
#[verifies("rule_review_row_refusal_stays_with_record", examples)]
fn the_records_of_the_lines_of_a_disputed_submission_refuse() {
    let proposals = [
        submission("sub_a", REQ_A, 1),
        changed_submission("sub_a", "req_b", 1),
        submission("sub_c", "req_c", 1),
    ];
    let reading = read_review_rows(StoredReviewRows {
        proposals: &proposals,
        dispositions: &[],
        withdrawals: &[],
    });
    let fault = sub_a_on_lines([0, 1]);
    for record in [REQ_A, "req_b"] {
        assert_eq!(
            reading
                .record(NodeType::Requirement, &id(record))
                .unwrap_err(),
            &fault
        );
    }
    assert!(reading.record(NodeType::Requirement, &id("req_c")).is_ok());
}
