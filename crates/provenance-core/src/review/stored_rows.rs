//! What the stored review rows of one scope mean when they are read as a
//! whole. A review write adds one final row for each submission, but a merge
//! of two branches keeps the rows that each branch added. The review reads
//! use these functions to decide what such rows say.

use crate::{StableId, Withdrawal};
use provenance_macros::rule;
use std::collections::HashSet;

/// A Withdrawal row that names no review submission of its scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawalWithoutSubmission {
    /// The position of the row in the rows that the caller gave.
    pub position: usize,
    pub withdrawal: StableId,
    pub proposal: StableId,
}

impl std::fmt::Display for WithdrawalWithoutSubmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "withdrawal {} names no review submission: {}",
            self.withdrawal.as_str(),
            self.proposal.as_str()
        )
    }
}

impl std::error::Error for WithdrawalWithoutSubmission {}

/// The ids of the review submissions that `withdrawals` withdraw, where a
/// submission that more than one row withdraws is one withdrawn submission.
#[rule("rule_repeated_withdrawals_read_as_one")]
pub fn withdrawn_submissions<'a>(
    is_submission: impl Fn(&str) -> bool,
    withdrawals: impl IntoIterator<Item = &'a Withdrawal>,
) -> Result<HashSet<&'a str>, WithdrawalWithoutSubmission> {
    let mut withdrawn = HashSet::new();
    for (position, withdrawal) in withdrawals.into_iter().enumerate() {
        let proposal = withdrawal.proposal_id.as_str();
        if !is_submission(proposal) {
            return Err(WithdrawalWithoutSubmission {
                position,
                withdrawal: withdrawal.id.clone(),
                proposal: withdrawal.proposal_id.clone(),
            });
        }
        withdrawn.insert(proposal);
    }
    Ok(withdrawn)
}
