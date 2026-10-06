//! Withdrawal records of review submissions.

use super::{readers::read_jsonl, StateStore};
use crate::shards;
use provenance_core::{ProposalType, ScopeId, Withdrawal};

impl StateStore {
    /// Reads the scope's Withdrawal records.
    pub fn list_withdrawals(&self, scope: &ScopeId) -> anyhow::Result<Vec<Withdrawal>> {
        read_jsonl(self, &shards::withdrawals_path(&self.layout, scope))
    }

    /// Writes the one Withdrawal of a review submission. Only the review seam
    /// writes Withdrawals, as it writes the Dispositions of review submissions.
    pub(crate) fn create_withdrawal(&self, withdrawal: Withdrawal) -> anyhow::Result<Withdrawal> {
        let scope = withdrawal.scope_id.clone();
        let path = shards::withdrawals_path(&self.layout, &scope);
        anyhow::ensure!(
            crate::review::guard::writer_allows(&path, "*"),
            "withdrawals of review submissions go through the review seam"
        );
        anyhow::ensure!(
            !withdrawal.actor.trim().is_empty(),
            "invalid withdrawal actor"
        );
        anyhow::ensure!(
            self.list_proposal_definitions(&scope)?
                .iter()
                .any(|proposal| proposal.id == withdrawal.proposal_id
                    && proposal.proposal_type == ProposalType::RecordRevision),
            "a withdrawal names a review submission"
        );
        self.mutate_jsonl_records(&path, |records: &mut Vec<Withdrawal>| {
            anyhow::ensure!(
                !records.iter().any(|record| record.id == withdrawal.id
                    || record.proposal_id == withdrawal.proposal_id),
                "the review submission is already withdrawn"
            );
            records.push(withdrawal.clone());
            records.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(withdrawal)
        })
    }
}
