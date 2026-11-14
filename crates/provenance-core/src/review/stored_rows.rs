//! What the stored Withdrawal rows of one scope mean when they are read as a
//! whole. A review write adds one Withdrawal for each submission, but a merge
//! of two branches keeps the rows that each branch added. The review reads
//! use this module to decide what such rows say.

use crate::{StableId, Withdrawal};
use provenance_macros::rule;
use std::collections::{HashMap, HashSet};

/// A Withdrawal row that the review reads cannot accept. Each position is
/// the position of a row in the rows that the caller gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WithdrawalRowFault {
    /// The row names no review submission of its scope.
    NoSubmission {
        position: usize,
        withdrawal: StableId,
        proposal: StableId,
    },
    /// The row has the id of an earlier row.
    RepeatedId {
        position: usize,
        first_position: usize,
        withdrawal: StableId,
    },
}

/// The ids of the review submissions that `withdrawals` withdraw, each one time, or the first
/// row that names no review submission or repeats the id of an earlier row.
#[rule("rule_repeated_withdrawals_read_as_one")]
pub fn withdrawn_submission_ids<'a>(
    is_submission: impl Fn(&str) -> bool,
    withdrawals: impl IntoIterator<Item = &'a Withdrawal>,
) -> Result<HashSet<&'a str>, WithdrawalRowFault> {
    let mut withdrawn = HashSet::new();
    let mut positions = HashMap::new();
    for (position, withdrawal) in withdrawals.into_iter().enumerate() {
        let proposal = withdrawal.proposal_id.as_str();
        if !is_submission(proposal) {
            return Err(WithdrawalRowFault::NoSubmission {
                position,
                withdrawal: withdrawal.id.clone(),
                proposal: withdrawal.proposal_id.clone(),
            });
        }
        if let Some(&first_position) = positions.get(withdrawal.id.as_str()) {
            return Err(WithdrawalRowFault::RepeatedId {
                position,
                first_position,
                withdrawal: withdrawal.id.clone(),
            });
        }
        positions.insert(withdrawal.id.as_str(), position);
        withdrawn.insert(proposal);
    }
    Ok(withdrawn)
}
