//! Each line of a review row takes part in each check of the record that it
//! names. A row ID on lines that disagree is one more fault and does not
//! hide its lines from the other checks, so one reading gives each fault.

use super::rows::{
    changed_submission, decision, decision_at, id, submission, submission_at, withdrawal,
    withdrawal_at, REQ_A,
};
use crate::review::{
    read_review_rows, ReviewRowFamily, ReviewRowFault, RowAt, StoredReviewRows, SubmissionLines,
};
use crate::{DispositionDecision, DispositionRecord, NodeType, ProposalCard, Withdrawal};
use provenance_macros::verifies;
use std::collections::HashSet;

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

/// The stored rows of the lines that `kept` holds, with the fixture index of
/// each row by family. The fixture lines are the submissions, then the
/// decisions, then the withdrawals.
struct State {
    proposals: Vec<ProposalCard>,
    dispositions: Vec<DispositionRecord>,
    withdrawals: Vec<Withdrawal>,
    origin: [Vec<usize>; 3],
}

impl State {
    fn new(fixture: &Self, kept: &[bool]) -> Self {
        fn keep<T: Clone>(rows: &[T], first: usize, kept: &[bool]) -> (Vec<T>, Vec<usize>) {
            (first..first + rows.len())
                .filter(|&line| kept[line])
                .map(|line| (rows[line - first].clone(), line))
                .unzip()
        }
        let decisions = fixture.proposals.len();
        let withdrawals = decisions + fixture.dispositions.len();
        let (proposals, submission_lines) = keep(&fixture.proposals, 0, kept);
        let (dispositions, decision_lines) = keep(&fixture.dispositions, decisions, kept);
        let (stored, withdrawal_lines) = keep(&fixture.withdrawals, withdrawals, kept);
        Self {
            proposals,
            dispositions,
            withdrawals: stored,
            origin: [submission_lines, decision_lines, withdrawal_lines],
        }
    }

    fn line(&self, at: RowAt) -> usize {
        let family = match at.family {
            ReviewRowFamily::Submission => 0,
            ReviewRowFamily::Decision => 1,
            ReviewRowFamily::Withdrawal => 2,
        };
        self.origin[family][at.position]
    }

    fn lines(&self, at: impl IntoIterator<Item = RowAt>) -> Vec<usize> {
        at.into_iter().map(|at| self.line(at)).collect()
    }

    /// Each fault, as a key that does not depend on the line positions,
    /// with the fixture lines that each option of its repair step removes.
    fn faults(&self) -> Vec<(String, Vec<Vec<usize>>)> {
        let reading = read_review_rows(StoredReviewRows {
            proposals: &self.proposals,
            dispositions: &self.dispositions,
            withdrawals: &self.withdrawals,
        });
        reading.faults().map(|fault| self.repair(fault)).collect()
    }

    fn repair(&self, fault: &ReviewRowFault) -> (String, Vec<Vec<usize>>) {
        let mut options = Vec::new();
        match fault {
            ReviewRowFault::IdOnTwoLines { id, lines, .. } => {
                for &kept in lines {
                    options.push(self.lines(lines.iter().copied().filter(|&at| at != kept)));
                }
                (format!("id {}", id.as_str()), options)
            }
            ReviewRowFault::DifferentResults {
                submission,
                decisions,
                ..
            } => {
                let result = |at: &RowAt| self.dispositions[at.position].decision;
                for kept in decisions {
                    let kept = result(kept);
                    let other = decisions.iter().copied().filter(|at| result(at) != kept);
                    options.push(self.lines(other));
                }
                (format!("results {}", submission.as_str()), options)
            }
            ReviewRowFault::AcceptedAndWithdrawn {
                submission,
                approvals,
                withdrawals,
            } => {
                options.push(self.lines(approvals.iter().copied()));
                options.push(self.lines(withdrawals.iter().copied()));
                (
                    format!("approved and withdrawn {}", submission.as_str()),
                    options,
                )
            }
            ReviewRowFault::OneCycleTwoSubmissions { cycle, submissions } => {
                for kept in 0..submissions.len() {
                    let others = submissions
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| *index != kept)
                        .flat_map(|(_, other)| other.lines.iter().copied());
                    options.push(self.lines(others));
                }
                let target = &self.proposals[submissions[0].submission.position]
                    .traceability
                    .target;
                (
                    format!("cycle {cycle} of {}", target.artifact_id.as_str()),
                    options,
                )
            }
            ReviewRowFault::NoSubmission { lines, .. } => {
                options.push(self.lines(lines.iter().copied()));
                let withdrawal = &self.withdrawals[lines[0].position].id;
                (format!("no submission {}", withdrawal.as_str()), options)
            }
        }
    }
}

fn removed(kept: &[bool], remove: &[usize]) -> Vec<bool> {
    let mut kept = kept.to_vec();
    for &line in remove {
        kept[line] = false;
    }
    kept
}

/// Property: for each set of the lines of the fixture, the repair step of a
/// fault, with each of its options, shows no fault that the reading before
/// it did not give, and the repair steps of all faults together leave no
/// fault.
#[test]
#[verifies("rule_check_reports_review_row_faults", property)]
#[verifies("rule_review_row_refusal_gives_repair", property)]
fn a_repair_step_shows_no_new_fault_and_all_steps_leave_none() {
    let fixture = State {
        proposals: vec![
            submission("sub_a", REQ_A, 1),
            submission("sub_b", REQ_A, 1),
            changed_submission("sub_a", REQ_A, 2),
            submission("sub_c", "req_b", 1),
        ],
        dispositions: vec![
            decision("dec_a", "sub_a", "accepted", "reviewer"),
            decision("dec_a", "sub_a", "accepted", "lead"),
            decision("dec_b", "sub_a", "rejected", "reviewer"),
            decision("dec_c", "sub_b", "accepted", "reviewer"),
        ],
        withdrawals: vec![
            withdrawal("wd_a", "sub_a", "agent"),
            withdrawal("wd_a", "sub_none", "agent"),
            withdrawal("wd_b", "sub_b", "agent"),
        ],
        origin: [Vec::new(), Vec::new(), Vec::new()],
    };
    let count = fixture.proposals.len() + fixture.dispositions.len() + fixture.withdrawals.len();
    for set in 0..1_u32 << count {
        let kept = (0..count)
            .map(|line| (set >> line) & 1 == 1)
            .collect::<Vec<_>>();
        let before = State::new(&fixture, &kept).faults();
        let keys = before.iter().map(|(key, _)| key).collect::<HashSet<_>>();
        let mut all_steps = Vec::new();
        for (key, options) in &before {
            for option in options {
                let after = State::new(&fixture, &removed(&kept, option)).faults();
                for (new, _) in &after {
                    assert!(keys.contains(new), "set {set:b}, repair of {key}: {new}");
                }
            }
            all_steps.extend(options[0].iter().copied());
        }
        let repaired = State::new(&fixture, &removed(&kept, &all_steps)).faults();
        assert!(repaired.is_empty(), "set {set:b}: {repaired:?}");
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
