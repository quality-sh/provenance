//! The review state of one record whose review rows agree.

use super::{submission_order, RecordRows, ReviewReading};
use crate::{DispositionRecord, ProposalCard, StableId};
use provenance_macros::rule;

/// The review state of one record, from the rows that read as one row.
#[derive(Debug, Clone, Copy)]
pub struct RecordReview<'r> {
    reading: &'r ReviewReading<'r>,
    /// The submissions of the record, in submission order.
    submissions: &'r [usize],
    other_decisions: &'r [(usize, usize)],
}

impl<'r> RecordReview<'r> {
    pub(super) fn new(reading: &'r ReviewReading<'r>, rows: Option<&'r RecordRows>) -> Self {
        Self {
            reading,
            submissions: rows.map_or(&[], |rows| rows.submissions.as_slice()),
            other_decisions: rows.map_or(&[], |rows| rows.other_decisions.as_slice()),
        }
    }

    /// The review submissions of the record, in submission order: the
    /// review cycle, then the ID.
    pub fn submissions(&self) -> impl Iterator<Item = &'r ProposalCard> + 'r {
        let proposals = self.reading.rows.proposals;
        self.submissions
            .iter()
            .map(move |&position| &proposals[position])
    }

    pub const fn has_submissions(&self) -> bool {
        !self.submissions.is_empty()
    }

    /// The decisions that name the record, each with its Proposal: the
    /// submissions in submission order, and the decisions of one submission
    /// sorted by the row ID. Each row is in the list one time.
    #[rule("rule_review_history_lists_each_row_once")]
    pub fn decisions(&self) -> Vec<(&'r DispositionRecord, &'r ProposalCard)> {
        let rows = self.reading.rows;
        let mut decisions = self
            .submissions
            .iter()
            .flat_map(|&submission| {
                let id = rows.proposals[submission].id.as_str();
                let (_, of) = &self.reading.submissions[id];
                of.decisions
                    .iter()
                    .map(move |&decision| (decision, submission))
            })
            .chain(self.other_decisions.iter().copied())
            .map(|(decision, proposal)| (&rows.dispositions[decision], &rows.proposals[proposal]))
            .collect::<Vec<_>>();
        decisions.sort_by(|(left, left_proposal), (right, right_proposal)| {
            (submission_order(left_proposal), left.id.as_str())
                .cmp(&(submission_order(right_proposal), right.id.as_str()))
        });
        decisions
    }

    fn rows_of(&self, submission: &StableId) -> Option<&'r super::SubmissionRows> {
        self.reading
            .submissions
            .get(submission.as_str())
            .map(|(_, rows)| rows)
    }

    /// Whether a decision names the submission.
    pub fn is_decided(&self, submission: &StableId) -> bool {
        self.rows_of(submission)
            .is_some_and(|rows| !rows.decisions.is_empty())
    }

    /// Whether a withdrawal names the submission. More than one withdrawal
    /// withdraws it one time.
    #[rule("rule_repeated_withdrawals_read_as_one")]
    pub fn is_withdrawn(&self, submission: &StableId) -> bool {
        self.rows_of(submission)
            .is_some_and(|rows| !rows.withdrawals.is_empty())
    }

    /// Whether the submission is accepted.
    pub fn is_accepted(&self, submission: &StableId) -> bool {
        self.reading.agreed_decision(submission) == Some(crate::DispositionDecision::Accepted)
    }

    /// The withdrawn submissions of the record, in submission order.
    pub fn withdrawn(&self) -> Vec<StableId> {
        self.submissions()
            .filter(|proposal| self.is_withdrawn(&proposal.id))
            .map(|proposal| proposal.id.clone())
            .collect()
    }

    /// Whether a later submission or a revision link replaces this
    /// submission.
    pub fn is_replaced(&self, proposal: &ProposalCard) -> bool {
        self.submissions().any(|candidate| {
            submission_order(candidate) > submission_order(proposal)
                || candidate.revises.as_ref() == Some(&proposal.id)
        })
    }

    /// The last submission in submission order that has no decision and no
    /// withdrawal and that names `revision`.
    pub fn pending_at(&self, revision: Option<&StableId>) -> Option<&'r ProposalCard> {
        self.submissions()
            .filter(|proposal| !self.is_decided(&proposal.id) && !self.is_withdrawn(&proposal.id))
            .filter(|proposal| {
                proposal
                    .record_revision
                    .as_ref()
                    .is_some_and(|binding| Some(&binding.revision) == revision)
            })
            .last()
    }

    /// The review cycle of the next submission of the record: one more than
    /// the number of its review submissions. `None` on overflow.
    pub fn next_cycle(&self) -> Option<u64> {
        u64::try_from(self.submissions.len()).ok()?.checked_add(1)
    }
}
