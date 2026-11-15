use crate::operations::reader::{Live, ReadContext, PAGE_BYTES};
use provenance_core::protocol::{take_page, EvidenceQuery, EvidenceResult, Stamped, StaleEvidence};
use provenance_core::{
    ImplementationBinding, RequirementReview, StableId, VerificationBinding, VerificationRun,
};
use provenance_macros::rule;
use std::collections::VecDeque;

use super::super::sites;

/// Everything standing behind one Rule, kept apart by kind.
///
/// Implementation bindings, verification bindings, and open reviews come
/// from the projection; verification runs stay cache JSONL.
/// `implementations` and `verifications` add the `#[rule]` and
/// `#[verifies]` attributes from the working-tree scan to the typed
/// bindings of the page, as impact does, and `scan_cut` says when that
/// scan stopped short. Review required says the Requirement was restated;
/// stale says the code carrying the evidence changed, and it is read from
/// a diff the caller names. Each of the four record lists carries its own
/// cut flag beside the top-level `has_more`. The two site lists stop at
/// the same limit and have no flag of their own, so `has_more` is the OR
/// of the four flags and of the two site cuts.
#[rule("rule_evidence_flags_each_cut_list")]
#[rule("rule_evidence_gives_scanned_sites")]
pub(super) async fn evidence(
    ctx: &ReadContext,
    request: EvidenceQuery,
) -> anyhow::Result<EvidenceResult> {
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let rule = StableId::new(request.rule.clone())?;
    let scope = ctx.snapshot().scope().clone();
    let snapshot = ctx.snapshot();
    let by_rule = [rule.as_str()];
    // Each list is one bounded page read: the page is chosen by id, so
    // the records that decode are the records the answer serves.
    let (implementation_bindings, implementation_bindings_has_more) = snapshot
        .table::<ImplementationBinding>()
        .page_by_field("rule_id", &by_rule, "", request.limit)
        .await?;
    let (verification_bindings, verification_bindings_has_more) = snapshot
        .table::<VerificationBinding>()
        .page_by_field("rule_id", &by_rule, "", request.limit)
        .await?;
    let (verification_runs, verification_runs_has_more) = ctx
        .live(Live::VerificationRuns)
        .read_runs(&scope, |_, next| newest_runs(next, &rule, request.limit))?;
    let latest_verification_run = verification_runs.first().cloned();
    // The table answers in id order; only the reviews still waiting on a
    // run are open, so the open-only cut is part of the candidate query.
    let (reviews, reviews_has_more) = snapshot
        .table::<RequirementReview>()
        .page_by_field("rule_id", &by_rule, "cleared_at IS NULL", request.limit)
        .await?;
    let review_required = !reviews.is_empty();

    let stale = request
        .base
        .map(|base| {
            let diff = ctx.live(Live::Diff);
            let (base, head) = diff.resolve_range(base, request.head)?;
            let graph = ctx.live(Live::Canonical).graph_evidence()?;
            diff.disturbed(base, head, std::slice::from_ref(&request.rule), &graph)
                .map(|found| StaleEvidence {
                    base: found.base,
                    head: found.head,
                    sites: found.sites,
                })
        })
        .transpose()?;
    let (scans, scan_cut) = ctx.live(Live::ScannedSites).scan_tree()?;
    let sites = sites::Evidence {
        scans: &scans,
        verifications: &verification_bindings,
        implementations: &implementation_bindings,
    }
    .affected_rule(ctx.repo(), rule);
    let (implementations, implementations_cut) = take_page(sites.implementations, request.limit);
    let (verifications, verifications_cut) = take_page(sites.verifications, request.limit);
    Ok(EvidenceResult {
        rule_id: request.rule,
        limit: request.limit,
        has_more: implementation_bindings_has_more
            || verification_bindings_has_more
            || verification_runs_has_more
            || reviews_has_more
            || implementations_cut
            || verifications_cut,
        implementations,
        verifications,
        scan_cut,
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

/// The native answer with the oldest runs of its run list left out until the
/// answer fits the response budget of its envelope, and the cut flagged.
/// The newest run stays in `latest_verification_run`.
pub(super) fn within_response(
    mut answer: Stamped<EvidenceResult>,
) -> anyhow::Result<Stamped<EvidenceResult>> {
    let mut excess = super::page::excess("evidence", &answer)?;
    let result = &mut answer.result;
    while excess > 0 {
        let Some(run) = result.verification_runs.pop() else {
            break;
        };
        // A run leaves with its bytes and the comma before it.
        let comma = usize::from(!result.verification_runs.is_empty());
        excess = excess.saturating_sub(serde_json::to_vec(&run)?.len() + comma);
        result.verification_runs_has_more = true;
        result.has_more = true;
    }
    Ok(answer)
}

/// The newest runs of `rule`, newest first, and whether the Rule has more
/// runs. The runs given stay inside `limit` and inside the bytes of one page,
/// as the page read of the runs counts them. While the file is read, only the
/// runs that can still be in the answer stay in memory.
fn newest_runs(
    next: &mut dyn FnMut() -> anyhow::Result<Option<(i64, VerificationRun)>>,
    rule: &StableId,
    limit: usize,
) -> anyhow::Result<(Vec<VerificationRun>, bool)> {
    // Oldest first, each run with its serialized bytes.
    let mut kept: VecDeque<(VerificationRun, usize)> = VecDeque::new();
    let mut bytes = 0;
    let mut matched = 0_usize;
    while let Some((_, run)) = next()? {
        if run.rule_id != *rule {
            continue;
        }
        matched += 1;
        let size = serde_json::to_vec(&run)?.len();
        let at = kept.partition_point(|(older, _)| start_order(older) < start_order(&run));
        kept.insert(at, (run, size));
        bytes += size;
        while kept.len() > limit || bytes > PAGE_BYTES {
            let Some((_, size)) = kept.pop_front() else {
                break;
            };
            bytes -= size;
        }
    }
    let has_more = matched > kept.len();
    Ok((
        kept.into_iter().rev().map(|(run, _)| run).collect(),
        has_more,
    ))
}

/// The order of the runs from the oldest start to the newest.
fn start_order(run: &VerificationRun) -> (i64, &str) {
    (run.started_at, run.id.as_str())
}
