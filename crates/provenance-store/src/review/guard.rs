//! Review state that a scope export file cannot hold, which a scope import and a scope export
//! refuse.

use crate::state_store::StateStore;
use provenance_core::{ProposalType, ScopeId};
use provenance_macros::rule;

/// One kind of stored review state that a scope export file cannot hold, in the order that the
/// guard checks it.
#[derive(Clone, Copy)]
enum UnportableState {
    /// The folder `review` below the scope folder.
    ReviewFolder,
    /// A review submission, a Discussion or a withdrawal.
    ReviewHistory,
    /// A row of `requirements/review.jsonl`.
    RequirementReviews,
}

impl UnportableState {
    const ALL: [Self; 3] = [
        Self::ReviewFolder,
        Self::ReviewHistory,
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
            Self::ReviewHistory => {
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

    /// The refusal for a scope that holds this state: what the scope holds, that a scope export
    /// file cannot hold it, and what the caller can do instead.
    fn refusal(self, scope: &ScopeId) -> anyhow::Error {
        let scope = scope.as_str();
        let held = match self {
            Self::ReviewFolder => format!("the review folder scopes/{scope}/review"),
            Self::ReviewHistory => "a review submission, a Discussion or a withdrawal".to_owned(),
            Self::RequirementReviews => format!(
                "a Requirement review record in scopes/{scope}/requirements/review.jsonl"
            ),
        };
        crate::refusal::refuse!(
            InvalidUpdate,
            "scope `{scope}` holds {held}, which a scope export file cannot hold, so an import \
             into this scope and an export of it are not possible; read and change the \
             Requirements, Rules and other records of this scope with the routes that \
             `provenance api --scope {scope}` lists"
        )
    }
}

impl StateStore {
    /// Refuses a scope import or a scope export of a scope that holds review state that a scope
    /// export file cannot hold.
    #[rule("rule_import_export_refuse_requirement_reviews")]
    pub fn ensure_review_portable(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.with_repository_publication(|held| {
            for state in UnportableState::ALL {
                if state.is_in(held, scope)? {
                    return Err(state.refusal(scope));
                }
            }
            Ok(())
        })
    }
}
