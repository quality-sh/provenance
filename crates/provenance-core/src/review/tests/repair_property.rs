//! A repair step of a review row fault shows no fault that the reading
//! before it did not give, so one reading gives each fault that a person
//! repairs.

use super::rows::{changed_submission, decision, submission, withdrawal, REQ_A};
use crate::review::{
    read_review_rows, ReviewReading, ReviewRowFamily, ReviewRowFault, RowAt, StoredReviewRows,
};
use crate::{DispositionRecord, NodeType, ProposalCard, StableId, Withdrawal};
use provenance_macros::verifies;
use std::collections::HashSet;

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

    /// Each fault, as a key that does not depend on the line positions and
    /// that holds the records of the fault, with the fixture lines that each
    /// option of its repair step removes.
    fn faults(&self) -> Vec<(String, Vec<Vec<usize>>)> {
        let reading = read_review_rows(StoredReviewRows {
            proposals: &self.proposals,
            dispositions: &self.dispositions,
            withdrawals: &self.withdrawals,
        });
        reading
            .faults()
            .map(|fault| {
                let (key, options) = self.repair(fault);
                let records = self.records(&reading, fault);
                (format!("{key} on {records:?}"), options)
            })
            .collect()
    }

    /// The records that the submissions of the lines of a fault name, in ID
    /// order. The record of a shared cycle is in the key of its repair.
    fn records(&self, reading: &ReviewReading<'_>, fault: &ReviewRowFault) -> Vec<&str> {
        let lines = match fault {
            ReviewRowFault::IdOnTwoLines { lines, .. } => lines.clone(),
            ReviewRowFault::DifferentResults { decisions, .. } => decisions.clone(),
            ReviewRowFault::AcceptedAndWithdrawn {
                approvals,
                withdrawals,
                ..
            } => [approvals.as_slice(), withdrawals].concat(),
            ReviewRowFault::OneCycleTwoSubmissions { .. } | ReviewRowFault::NoSubmission { .. } => {
                Vec::new()
            }
        };
        let mut records = self
            .proposals
            .iter()
            .map(|proposal| &proposal.traceability.target.artifact_id)
            .filter(|&record| {
                lines.iter().any(|&at| {
                    reading.submission_names(self.submission_of(at), NodeType::Requirement, record)
                })
            })
            .map(StableId::as_str)
            .collect::<Vec<_>>();
        records.sort_unstable();
        records.dedup();
        records
    }

    /// The submission ID that a line has or names.
    fn submission_of(&self, at: RowAt) -> &StableId {
        match at.family {
            ReviewRowFamily::Submission => &self.proposals[at.position].id,
            ReviewRowFamily::Decision => &self.dispositions[at.position].proposal_id,
            ReviewRowFamily::Withdrawal => &self.withdrawals[at.position].proposal_id,
        }
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
/// fault, with each of its options, shows no fault on a record that the
/// reading before it did not give, and the repair steps of all faults
/// together leave no fault. The lines of `sub_a` name two records.
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
            submission("sub_a", "req_b", 1),
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
