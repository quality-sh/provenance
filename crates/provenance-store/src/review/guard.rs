//! Review state that import and export cannot carry yet.

impl crate::state_store::StateStore {
    /// Full-scope import and export cannot yet carry review state.
    pub fn ensure_review_portable(&self, scope: &provenance_core::ScopeId) -> anyhow::Result<()> {
        self.with_repository_publication(|held| {
            anyhow::ensure!(
                !held
                    .layout
                    .scopes_dir()
                    .join(scope.as_str())
                    .join("review")
                    .try_exists()?,
                "review-bearing scopes require lossless import/export support"
            );
            let has_review_state = !held.list_discussions(scope)?.is_empty()
                || !held.list_withdrawals(scope)?.is_empty()
                || held
                    .list_proposal_definitions(scope)?
                    .iter()
                    .any(|proposal| {
                        proposal.proposal_type == provenance_core::ProposalType::RecordRevision
                    });
            anyhow::ensure!(
                !has_review_state,
                "review-bearing scopes require lossless import/export support"
            );
            Ok(())
        })
    }
}
