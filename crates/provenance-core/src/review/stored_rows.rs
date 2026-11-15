//! What the stored review rows of one scope mean when they are read as a
//! whole. The review rows are the review submissions, the decisions on them
//! and their withdrawals. A review write adds one row at a time, but a merge
//! of two branches keeps the rows that each branch added, and a person can
//! keep both sides of a conflict. Each read, write and check of the review
//! state uses [`read_review_rows`], so only this module decides what two
//! review rows mean.
//!
//! Rows that say the same thing read as one. Rows that disagree give a
//! [`ReviewRowFault`] to each record that they name, and the other records
//! read as usual.

mod reading;
mod record_review;
mod row_groups;

use crate::{
    DispositionDecision, DispositionRecord, NodeType, ProposalCard, ProposalType, StableId,
    Withdrawal,
};
use provenance_macros::rule;
use std::collections::HashMap;

pub use record_review::RecordReview;

/// The file of a review row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReviewRowFamily {
    Submission,
    Decision,
    Withdrawal,
}

/// One stored row: its family and its position in the rows of that family
/// that the caller gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RowAt {
    pub family: ReviewRowFamily,
    pub position: usize,
}

/// The stored rows of one scope, each family in line order.
#[derive(Debug, Clone, Copy)]
pub struct StoredReviewRows<'a> {
    /// Every Proposal row of the scope. Only the review submissions among
    /// them are review rows.
    pub proposals: &'a [ProposalCard],
    /// Every Disposition row of the scope. Only the decisions on review
    /// submissions are review rows.
    pub dispositions: &'a [DispositionRecord],
    pub withdrawals: &'a [Withdrawal],
}

/// Whether a Proposal is a review submission. Only the review submissions,
/// the decisions on them and their withdrawals are review rows.
pub fn is_review_submission(proposal: &ProposalCard) -> bool {
    proposal.proposal_type == ProposalType::RecordRevision
}

/// One review submission of a record, with the lines that remove it.
///
/// The lines are each line of the submission and each decision line and
/// withdrawal line that names it, by family, then in line order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionLines {
    /// The first line of the submission.
    pub submission: RowAt,
    pub lines: Vec<RowAt>,
}

/// Review rows that do not agree.
///
/// Each fault holds each line that a person removes to repair it, so that
/// one repair is sufficient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewRowFault {
    /// One row ID is on more than one line, and the lines do not have equal
    /// contents. The person keeps one of the lines and removes the others.
    IdOnTwoLines {
        id: StableId,
        /// Each line of the ID, in line order.
        lines: Vec<RowAt>,
        /// The first line whose contents are not the contents of the first
        /// line.
        second: RowAt,
    },
    /// The decisions on one submission have different results. The person
    /// removes each decision line with a result that they do not keep.
    DifferentResults {
        submission: StableId,
        /// Each decision line that names the submission, in line order.
        decisions: Vec<RowAt>,
        /// The first decision whose result is not the result of the first
        /// decision.
        second: RowAt,
    },
    /// One submission has an approval and a withdrawal. The person removes
    /// each approval line or each withdrawal line.
    AcceptedAndWithdrawn {
        submission: StableId,
        /// Each approval line that names the submission, in line order.
        approvals: Vec<RowAt>,
        /// Each withdrawal line that names the submission, in line order.
        withdrawals: Vec<RowAt>,
    },
    /// More than one submission of one record has one review cycle. The
    /// person keeps one submission and removes the lines of each other.
    OneCycleTwoSubmissions {
        cycle: u64,
        /// The submissions with the cycle, in line order.
        submissions: Vec<SubmissionLines>,
    },
    /// A withdrawal names no review submission, so it names no record. The
    /// person removes each line of the withdrawal.
    NoSubmission {
        proposal: StableId,
        /// Each line of the withdrawal, in line order.
        lines: Vec<RowAt>,
    },
}

impl ReviewRowFault {
    /// The row whose line the refusal names.
    pub fn at(&self) -> RowAt {
        match self {
            Self::IdOnTwoLines { second, .. } | Self::DifferentResults { second, .. } => *second,
            Self::AcceptedAndWithdrawn { withdrawals, .. } => withdrawals[0],
            Self::OneCycleTwoSubmissions { submissions, .. } => submissions[1].submission,
            Self::NoSubmission { lines, .. } => lines[0],
        }
    }
}

/// The review cycle that the key of a review submission names, or `None`
/// for a key in another form.
pub fn review_cycle(proposal: &ProposalCard) -> Option<u64> {
    let target = &proposal.traceability.target;
    let prefix = format!(
        "review:{}:{}:",
        NodeType::from(target.artifact_type).as_str(),
        target.artifact_id.as_str()
    );
    proposal
        .proposal_key
        .strip_prefix(&prefix)
        .and_then(|cycle| cycle.parse().ok())
}

/// The decisions and withdrawals that name one submission, as positions.
#[derive(Debug, Default)]
struct SubmissionRows {
    decisions: Vec<usize>,
    withdrawals: Vec<usize>,
}

/// The rows of one record, as positions.
#[derive(Debug, Default)]
struct RecordRows {
    /// The submissions that read as one row, in submission order.
    submissions: Vec<usize>,
    /// The decisions on other Proposals that name the record, each with the
    /// position of its Proposal.
    other_decisions: Vec<(usize, usize)>,
    /// The first fault of the record, as a position in the faults.
    fault: Option<usize>,
}

/// The rows that read as one row, as positions in line order.
#[derive(Debug, Default)]
pub struct RowsOnce {
    pub proposals: Vec<usize>,
    pub dispositions: Vec<usize>,
    pub withdrawals: Vec<usize>,
}

/// What the stored review rows of one scope mean.
#[derive(Debug)]
pub struct ReviewReading<'a> {
    rows: StoredReviewRows<'a>,
    once: RowsOnce,
    faults: Vec<ReviewRowFault>,
    scope_fault: Option<usize>,
    /// The review submission IDs, including an ID on two lines that
    /// disagree.
    submission_ids: HashMap<&'a str, Vec<usize>>,
    /// The submissions that read as one row, by ID, with their rows.
    submissions: HashMap<&'a str, (usize, SubmissionRows)>,
    /// The rows of each record, by the name of its kind, then by its ID.
    records: HashMap<&'static str, HashMap<&'a str, RecordRows>>,
}

/// A record: the name of its kind and its ID.
type RecordKey<'a> = (&'static str, &'a str);

fn target(proposal: &ProposalCard) -> RecordKey<'_> {
    let target = &proposal.traceability.target;
    (
        NodeType::from(target.artifact_type).as_str(),
        target.artifact_id.as_str(),
    )
}

/// The order of submissions: the review cycle, then the ID. A submission
/// whose key names no cycle is at the end.
fn submission_order(proposal: &ProposalCard) -> (u64, &str) {
    (
        review_cycle(proposal).unwrap_or(u64::MAX),
        proposal.id.as_str(),
    )
}

/// Reads the stored review rows of one scope as a whole.
#[rule("rule_equal_review_row_lines_read_as_one")]
pub fn read_review_rows(rows: StoredReviewRows<'_>) -> ReviewReading<'_> {
    let mut reading = ReviewReading {
        rows,
        once: RowsOnce::default(),
        faults: Vec::new(),
        scope_fault: None,
        submission_ids: HashMap::new(),
        submissions: HashMap::new(),
        records: HashMap::new(),
    };
    reading.read_submissions();
    reading.read_decisions();
    reading.read_withdrawals();
    reading.read_records();
    reading
}

impl ReviewReading<'_> {
    /// Whether a Proposal ID names a review submission, including an ID on
    /// two lines that disagree.
    pub fn is_submission(&self, proposal: &StableId) -> bool {
        self.submission_ids.contains_key(proposal.as_str())
    }

    /// The review state of one record, or the first fault of its rows.
    pub fn record(
        &self,
        kind: NodeType,
        id: &StableId,
    ) -> Result<RecordReview<'_>, &ReviewRowFault> {
        let rows = self
            .records
            .get(kind.as_str())
            .and_then(|ids| ids.get(id.as_str()));
        if let Some(fault) = rows.and_then(|rows| rows.fault) {
            return Err(&self.faults[fault]);
        }
        Ok(RecordReview::new(self, rows))
    }

    /// The review submission with this ID, or the first fault of a record
    /// that a line of the ID names. `None` when no review submission has the
    /// ID. A write that names a submission asks this before it reads the
    /// submission.
    pub fn submission(&self, id: &StableId) -> Result<Option<&ProposalCard>, &ReviewRowFault> {
        for (kind, record) in self.records_of_submission(id.as_str()) {
            let rows = self.records.get(kind).and_then(|ids| ids.get(record));
            if let Some(fault) = rows.and_then(|rows| rows.fault) {
                return Err(&self.faults[fault]);
            }
        }
        Ok(self
            .submissions
            .get(id.as_str())
            .map(|&(position, _)| &self.rows.proposals[position]))
    }

    /// The first withdrawal that names no review submission, in line order.
    pub fn scope_fault(&self) -> Option<&ReviewRowFault> {
        self.scope_fault.map(|fault| &self.faults[fault])
    }

    /// Each fault, in the order of the reading: an ID on lines that
    /// disagree (submissions, then decisions, then withdrawals, each in line
    /// order) and a withdrawal that names no review submission, then for
    /// each record in the line order of its first submission: each review
    /// cycle that more than one submission has, then submission by
    /// submission in line order, decisions with different results and an
    /// approval with a withdrawal.
    pub fn faults(&self) -> impl Iterator<Item = &ReviewRowFault> {
        self.faults.iter()
    }

    /// The rows that read as one row, as positions in line order. The other
    /// lines of an ID on lines that agree are not in it. No line of an ID on
    /// lines that disagree is in it, and no decision or withdrawal that
    /// names a submission with such an ID: these rows are a part of the
    /// fault of the submission.
    pub const fn rows_once(&self) -> &RowsOnce {
        &self.once
    }

    /// The result of the decisions on one submission when they agree.
    #[rule("rule_same_result_decisions_read_as_one")]
    pub fn agreed_decision(&self, submission: &StableId) -> Option<DispositionDecision> {
        let (_, rows) = self.submissions.get(submission.as_str())?;
        let mut results = rows
            .decisions
            .iter()
            .map(|&position| self.rows.dispositions[position].decision);
        let first = results.next()?;
        results.all(|result| result == first).then_some(first)
    }
}
