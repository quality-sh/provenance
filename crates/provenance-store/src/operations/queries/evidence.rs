use camino::Utf8Path;
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, EvidenceQuery, EvidenceResult, StaleEvidence,
};
use provenance_core::{ScopeId, StableId};
use provenance_store::state_store::StateStore;

use super::{bindings::Bindings, stale};

/// Everything standing behind one Rule, kept apart by kind.
///
/// Implementation bindings, verification bindings, and verification runs are
/// separate records and stay separate here. Review required says the
/// Requirement was restated; stale says the code carrying the evidence
/// changed, and it is read from a diff the caller names.
pub(super) fn evidence(
    repo: &Utf8Path,
    store: &StateStore,
    scope: &ScopeId,
    request: EvidenceQuery,
) -> anyhow::Result<EvidenceResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let rule = StableId::new(request.rule.clone())?;
    let bindings = Bindings::load(store, scope, request.include_retired)?;
    let implementations = bindings
        .implementations
        .into_iter()
        .filter(|binding| binding.rule_id == rule)
        .take(request.limit + 1)
        .collect::<Vec<_>>();
    let verifications = bindings
        .verifications
        .into_iter()
        .filter(|binding| binding.rule_id == rule)
        .take(request.limit + 1)
        .collect::<Vec<_>>();
    let mut runs = store
        .list_verification_runs(scope)?
        .into_iter()
        .filter(|run| run.rule_id == rule)
        .collect::<Vec<_>>();
    runs.sort_by(|left, right| {
        right
            .started_at
            .cmp(&left.started_at)
            .then_with(|| right.id.as_str().cmp(left.id.as_str()))
    });
    let latest_verification_run = runs.first().cloned();
    runs.truncate(request.limit + 1);
    let mut reviews = store
        .open_requirement_reviews(scope)?
        .into_iter()
        .filter(|review| review.rule_id == rule)
        .collect::<Vec<_>>();
    reviews.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    let review_required = !reviews.is_empty();
    reviews.truncate(request.limit + 1);

    let (implementation_bindings, cut_implementations) = take_page(implementations, request.limit);
    let (verification_bindings, cut_verifications) = take_page(verifications, request.limit);
    let (verification_runs, cut_runs) = take_page(runs, request.limit);
    let (reviews, cut_reviews) = take_page(reviews, request.limit);
    let stale = request
        .base
        .map(|base| {
            stale::disturbed(
                repo,
                scope,
                base,
                request.head,
                std::slice::from_ref(&request.rule),
                request.include_retired,
            )
            .map(|found| StaleEvidence {
                base: found.base,
                head: found.head,
                sites: found.sites,
            })
        })
        .transpose()?;
    Ok(EvidenceResult {
        rule_id: request.rule,
        limit: request.limit,
        has_more: cut_implementations || cut_verifications || cut_runs || cut_reviews,
        implementation_bindings,
        verification_bindings,
        verification_runs,
        latest_verification_run,
        review_required,
        reviews,
        stale,
    })
}
