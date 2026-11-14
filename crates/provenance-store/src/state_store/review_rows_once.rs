//! The stored review rows of a scope, each ID one time, for the list reads,
//! the checks of the ideation records and the cache catch-up. The core
//! reading decides which rows read as one: an ID on two equal lines gives
//! one row, and no line of an ID on two lines that disagree is in the list.
//! The read of the record that such rows name refuses with the lines.

use super::shared_parses::read_jsonl_shared;
use super::StateStore;
use crate::shards;
use provenance_core::{
    review::{read_review_rows, StoredReviewRows},
    DispositionRecord, ProposalCard, ScopeId, Withdrawal,
};

/// The rows at `keep`, positions in line order.
fn select<T>(rows: Vec<T>, keep: &[usize]) -> Vec<T> {
    let mut keep = keep.iter().copied().peekable();
    rows.into_iter()
        .enumerate()
        .filter_map(|(position, row)| {
            (keep.peek() == Some(&position)).then(|| {
                keep.next();
                row
            })
        })
        .collect()
}

/// The Proposal rows of the Proposal file, each review submission one time.
pub(super) fn proposals_once(proposals: Vec<ProposalCard>) -> Vec<ProposalCard> {
    let keep = read_review_rows(StoredReviewRows {
        proposals: &proposals,
        dispositions: &[],
        withdrawals: &[],
    })
    .rows_once()
    .proposals
    .clone();
    select(proposals, &keep)
}

/// The Disposition rows of the Disposition file, each decision on a review
/// submission one time. `proposals` are the rows of the Proposal file.
pub(super) fn dispositions_once(
    proposals: &[ProposalCard],
    dispositions: Vec<DispositionRecord>,
) -> Vec<DispositionRecord> {
    let keep = read_review_rows(StoredReviewRows {
        proposals,
        dispositions: &dispositions,
        withdrawals: &[],
    })
    .rows_once()
    .dispositions
    .clone();
    select(dispositions, &keep)
}

/// The Withdrawal rows, each ID one time.
pub(super) fn withdrawals_once(withdrawals: Vec<Withdrawal>) -> Vec<Withdrawal> {
    let keep = read_review_rows(StoredReviewRows {
        proposals: &[],
        dispositions: &[],
        withdrawals: &withdrawals,
    })
    .rows_once()
    .withdrawals
    .clone();
    select(withdrawals, &keep)
}

impl StateStore {
    /// The rows of the Disposition file of `scope`, each decision on a
    /// review submission one time.
    pub(super) fn disposition_file_rows_once(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<DispositionRecord>> {
        let proposals: Vec<ProposalCard> =
            read_jsonl_shared(self, &shards::proposal_cards_path(&self.layout, scope))?;
        let dispositions = read_jsonl_shared(self, &shards::dispositions_path(&self.layout, scope))?;
        Ok(dispositions_once(&proposals, dispositions))
    }

    /// The rows of the Proposal file of `scope`, each review submission one
    /// time.
    pub(super) fn proposal_file_rows_once(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ProposalCard>> {
        Ok(proposals_once(read_jsonl_shared(
            self,
            &shards::proposal_cards_path(&self.layout, scope),
        )?))
    }
}
