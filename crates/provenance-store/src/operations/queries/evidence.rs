use crate::operations::reader::{Live, ReadContext};
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, EvidenceQuery, EvidenceResult, StaleEvidence,
};
use provenance_core::{ImplementationBinding, RequirementReview, StableId, VerificationBinding};
use provenance_macros::rule;

/// Everything standing behind one Rule, kept apart by kind.
///
/// Implementation bindings, verification bindings, and open reviews come
/// from the projection; verification runs stay cache JSONL. Review
/// required says the Requirement was restated; stale says the code
/// carrying the evidence changed, and it is read from a diff the caller
/// names. Each of the four lists carries its own cut flag beside the
/// top-level `has_more`, which stays the OR of the four.
#[rule("rule_evidence_flags_each_cut_list")]
pub(super) async fn evidence(
    ctx: &ReadContext,
    request: EvidenceQuery,
) -> anyhow::Result<EvidenceResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let rule = StableId::new(request.rule.clone())?;
    let scope = ctx.snapshot().scope().clone();
    let include_retired = request.include_retired;
    let snapshot = ctx.snapshot();
    let by_rule = [rule.as_str()];
    let implementations = snapshot
        .table::<ImplementationBinding>()
        .by_field("rule_id", &by_rule, include_retired)
        .await?
        .into_iter()
        .take(request.limit + 1)
        .collect::<Vec<_>>();
    let verifications = snapshot
        .table::<VerificationBinding>()
        .by_field("rule_id", &by_rule, include_retired)
        .await?
        .into_iter()
        .take(request.limit + 1)
        .collect::<Vec<_>>();
    let mut runs = ctx
        .live(Live::VerificationRuns)
        .runs(&scope)?
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
    // The table answers in id order; only the reviews still waiting on a
    // run are open.
    let mut reviews = snapshot
        .table::<RequirementReview>()
        .by_field("rule_id", &by_rule, include_retired)
        .await?
        .into_iter()
        .filter(|review| review.cleared_at.is_none())
        .collect::<Vec<_>>();
    let review_required = !reviews.is_empty();
    reviews.truncate(request.limit + 1);

    let (implementation_bindings, implementation_bindings_has_more) =
        take_page(implementations, request.limit);
    let (verification_bindings, verification_bindings_has_more) =
        take_page(verifications, request.limit);
    let (verification_runs, verification_runs_has_more) = take_page(runs, request.limit);
    let (reviews, reviews_has_more) = take_page(reviews, request.limit);
    let stale = request
        .base
        .map(|base| {
            let diff = ctx.live(Live::Diff);
            let (base, head) = diff.resolve_range(base, request.head)?;
            let graph = ctx
                .live(Live::Canonical)
                .graph_evidence(&scope, include_retired)?;
            diff.disturbed(base, head, std::slice::from_ref(&request.rule), &graph)
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
        has_more: implementation_bindings_has_more
            || verification_bindings_has_more
            || verification_runs_has_more
            || reviews_has_more,
        implementation_bindings,
        verification_bindings,
        verification_runs,
        latest_verification_run,
        review_required,
        reviews,
        stale,
        implementation_bindings_has_more,
        verification_bindings_has_more,
        verification_runs_has_more,
        reviews_has_more,
    })
}
