use crate::operations::reader::{Live, ReadContext};
use provenance_core::coverage::{EvidenceDiffSite, EvidenceDiffState, EvidenceDiffSummary};
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, StaleQuery, StaleResult,
};
use provenance_core::ScopeId;

/// What a commit range did to the code carrying graph evidence.
///
/// Stale is read from a diff, never guessed from the working tree, so every
/// caller names a base. `head` defaults to the current commit. The graph
/// evidence the diff is read against comes from canonical shards.
pub(super) fn stale(
    ctx: &ReadContext,
    scope: &ScopeId,
    request: StaleQuery,
) -> anyhow::Result<StaleResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let graph = ctx
        .live(Live::Canonical)
        .graph_evidence(scope, request.include_retired)?;
    let found =
        ctx.live(Live::Diff)
            .disturbed(request.base, request.head, &request.rules, &graph)?;
    let summary = summarize(&found.sites);
    let (sites, has_more) = take_page(found.sites, request.limit);
    Ok(StaleResult {
        base: found.base,
        head: found.head,
        files_changed: found.files_changed,
        summary,
        limit: request.limit,
        has_more,
        sites,
    })
}

/// Counts the states a filtered page still stands for.
fn summarize(sites: &[EvidenceDiffSite]) -> EvidenceDiffSummary {
    let mut summary = EvidenceDiffSummary {
        total_sites: sites.len(),
        ..EvidenceDiffSummary::default()
    };
    for site in sites {
        match site.state {
            EvidenceDiffState::Untouched => summary.untouched += 1,
            EvidenceDiffState::Touched => summary.touched += 1,
            EvidenceDiffState::Moved => summary.moved += 1,
            EvidenceDiffState::Gone => summary.gone += 1,
        }
    }
    summary
}
