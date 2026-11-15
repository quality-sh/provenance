//! Review state that a scope export file cannot hold, which a scope import and a scope export
//! refuse.

use crate::state_store::StateStore;
use provenance_core::ScopeId;
use provenance_macros::rule;

/// One kind of stored review state that a scope export file cannot hold, in the order that the
/// guard checks it.
#[derive(Clone, Copy)]
enum UnportableState {
    /// A review submission, a Discussion or a withdrawal.
    ReviewHistory,
    /// A row of `requirements/review.jsonl`.
    RequirementReviews,
}

impl UnportableState {
    const ALL: [Self; 2] = [Self::ReviewHistory, Self::RequirementReviews];

    fn is_in(self, held: &StateStore, scope: &ScopeId) -> anyhow::Result<bool> {
        Ok(match self {
            Self::ReviewHistory => {
                !held.list_discussions(scope)?.is_empty()
                    || held.has_review_rows(scope)?
            }
            Self::RequirementReviews => !held.list_requirement_reviews(scope)?.is_empty(),
        })
    }

    /// The refusal for a scope that holds this state: what the scope holds, that a scope export
    /// file cannot hold it, and what the caller can do instead.
    fn refusal(self, scope: &ScopeId) -> anyhow::Error {
        let scope = scope.as_str();
        let held = match self {
            Self::ReviewHistory => "a review submission, a Discussion or a withdrawal".to_owned(),
            Self::RequirementReviews => {
                format!("a Requirement review record in scopes/{scope}/requirements/review.jsonl")
            }
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
    #[rule("rule_import_export_refuse_review_data")]
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
