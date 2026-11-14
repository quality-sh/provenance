//! Review state that import and export cannot carry yet.

use crate::state_store::StateStore;
use provenance_core::{ProposalType, ScopeId};
use provenance_macros::rule;

/// The refusal for review submissions, Discussions, withdrawals and the review folder.
const REVIEW_STATE: &str = "review-bearing scopes require lossless import/export support";

/// One kind of stored review state that a scope export file cannot carry, in the order that
/// the guard checks it.
#[derive(Clone, Copy)]
enum UnportableState {
    ReviewFolder,
    ReviewRecords,
    RequirementReviews,
}

impl UnportableState {
    const ALL: [Self; 3] = [
        Self::ReviewFolder,
        Self::ReviewRecords,
        Self::RequirementReviews,
    ];

    fn is_in(self, held: &StateStore, scope: &ScopeId) -> anyhow::Result<bool> {
        Ok(match self {
            Self::ReviewFolder => held
                .layout
                .scopes_dir()
                .join(scope.as_str())
                .join("review")
                .try_exists()?,
            Self::ReviewRecords => {
                !held.list_discussions(scope)?.is_empty()
                    || !held.list_withdrawals(scope)?.is_empty()
                    || held
                        .list_proposal_definitions(scope)?
                        .iter()
                        .any(|proposal| proposal.proposal_type == ProposalType::RecordRevision)
            }
            Self::RequirementReviews => !held.list_requirement_reviews(scope)?.is_empty(),
        })
    }

    fn refusal(self, scope: &ScopeId) -> String {
        match self {
            Self::ReviewFolder | Self::ReviewRecords => REVIEW_STATE.to_owned(),
            Self::RequirementReviews => format!(
                "scope `{}` has Requirement review records, which an import or an export \
                 cannot carry; read and change the records of this scope through the API",
                scope.as_str()
            ),
        }
    }
}

impl StateStore {
    /// Refuses a full-scope import or export of a scope that holds review state that the scope
    /// export file cannot carry.
    #[rule("rule_import_export_refuse_requirement_reviews")]
    pub fn ensure_review_portable(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.with_repository_publication(|held| {
            for state in UnportableState::ALL {
                crate::write_error::ensure!(
                    InvalidUpdate,
                    !state.is_in(held, scope)?,
                    "{}",
                    state.refusal(scope)
                );
            }
            Ok(())
        })
    }
}
