use provenance_core::{RequirementReview, ScopeId, StableId};
use provenance_store::state_store::{
    requirement_statement_changes, StateStore, TypedResourceKind, TypedSpecResult,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// What the current state of a Rule's evidence asks a reader to do.
#[derive(Serialize)]
pub(super) struct RuleEvidence {
    review_required: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reasons: Vec<ReviewReason>,
}

/// One restated Requirement that put this Rule's evidence up for review.
#[derive(Debug, Clone, Serialize)]
pub(super) struct ReviewReason {
    pub requirement: StableId,
    pub field: String,
    pub before: String,
    pub after: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changed_at: Option<i64>,
}

impl ReviewReason {
    /// Two reasons match when they describe the same restatement.
    fn same_change(&self, other: &Self) -> bool {
        self.requirement == other.requirement
            && self.field == other.field
            && self.before == other.before
            && self.after == other.after
    }
}

/// Every Rule this plan should speak about, and why.
pub(super) struct Reviews {
    by_rule: BTreeMap<String, Vec<ReviewReason>>,
    pub(super) rules: Vec<StableId>,
}

impl Reviews {
    pub(super) fn evidence(&self, rule: &StableId) -> RuleEvidence {
        let reasons = self.by_rule.get(rule.as_str()).cloned().unwrap_or_default();
        RuleEvidence {
            review_required: !reasons.is_empty(),
            reasons,
        }
    }
}

impl RuleEvidence {
    pub(super) const fn review_required(&self) -> bool {
        self.review_required
    }

    pub(super) fn reasons(&self) -> &[ReviewReason] {
        &self.reasons
    }
}

/// Collects the reviews already on file plus the ones this plan would raise.
///
/// Records written by a previous apply are the standing answer. The planned
/// change adds what restating the Requirement now would ask for.
pub(super) fn reviews(
    store: &StateStore,
    scope: &ScopeId,
    reconciliation: &TypedSpecResult,
) -> anyhow::Result<Reviews> {
    let open = store.open_requirement_reviews(scope)?;
    let mut by_rule: BTreeMap<String, Vec<ReviewReason>> = BTreeMap::new();
    let mut rules = Vec::new();
    for review in &open {
        push_reason(&mut by_rule, &review.rule_id, recorded(review));
    }
    for requirement in requirement_ids(&open) {
        for rule in store.rule_ids_for_requirement(scope, &requirement)? {
            push_unique(&mut rules, rule);
        }
    }
    for change in requirement_statement_changes(&reconciliation.resources) {
        let mut affected = store.rule_ids_for_requirement(scope, &change.requirement_id)?;
        for resource in reconciliation.resources.iter().filter(|resource| {
            resource.kind == TypedResourceKind::Rule
                && resource.parent.as_deref() == Some(change.requirement_key.as_str())
        }) {
            push_unique(&mut affected, resource.id.clone());
        }
        for rule in affected {
            push_reason(
                &mut by_rule,
                &rule,
                ReviewReason {
                    requirement: change.requirement_id.clone(),
                    field: change.field.clone(),
                    before: change.before.clone(),
                    after: change.after.clone(),
                    changed_at: None,
                },
            );
            push_unique(&mut rules, rule);
        }
    }
    Ok(Reviews { by_rule, rules })
}

fn recorded(review: &RequirementReview) -> ReviewReason {
    ReviewReason {
        requirement: review.requirement_id.clone(),
        field: review.field.clone(),
        before: review.before.clone(),
        after: review.after.clone(),
        changed_at: Some(review.changed_at),
    }
}

fn requirement_ids(reviews: &[RequirementReview]) -> Vec<StableId> {
    let mut ids = Vec::new();
    for review in reviews {
        push_unique(&mut ids, review.requirement_id.clone());
    }
    ids
}

fn push_reason(
    by_rule: &mut BTreeMap<String, Vec<ReviewReason>>,
    rule: &StableId,
    reason: ReviewReason,
) {
    let reasons = by_rule.entry(rule.as_str().to_string()).or_default();
    if !reasons.iter().any(|existing| existing.same_change(&reason)) {
        reasons.push(reason);
    }
}

fn push_unique(ids: &mut Vec<StableId>, id: StableId) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}
