use crate::{cache, layout::ProvenanceLayout};
use camino::Utf8Path;
use provenance_core::coverage::{EvidenceDiffSite, EvidenceDiffState, EvidenceDiffSummary};
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, LiveConstituent, StaleQuery, StaleResult,
};
use provenance_core::ScopeId;

use crate::stale::{gate, git};

/// What a commit range did to the code carrying graph evidence.
///
/// Stale is read from a diff, never guessed from the working tree, so every
/// caller names a base. `head` defaults to the current commit.
pub(super) struct Disturbed {
    pub base: String,
    pub head: String,
    pub files_changed: usize,
    pub sites: Vec<EvidenceDiffSite>,
}

pub(super) fn disturbed(
    repo: &Utf8Path,
    scope: &ScopeId,
    base: String,
    head: Option<String>,
    rules: &[String],
    include_retired: bool,
) -> anyhow::Result<Disturbed> {
    let (base, head) = git::resolve_range(
        repo,
        Some(base),
        Some(head.unwrap_or_else(|| "HEAD".to_string())),
        None,
    )?;
    let graph = cache::graph_evidence(
        &ProvenanceLayout::new(repo.to_path_buf()),
        scope,
        include_retired,
    )?;
    disturbed_body(repo, base, head, rules, &graph)
}

/// The guarded variant: the caller already holds the publication guard,
/// so the graph evidence is read without requesting the lock again.
pub(super) fn disturbed_under_guard(
    guard: &crate::publication::guard::PublicationGuard,
    repo: &Utf8Path,
    scope: &ScopeId,
    base: String,
    head: Option<String>,
    rules: &[String],
    include_retired: bool,
) -> anyhow::Result<Disturbed> {
    let (base, head) = git::resolve_range(
        repo,
        Some(base),
        Some(head.unwrap_or_else(|| "HEAD".to_string())),
        None,
    )?;
    let graph = cache::graph_evidence_under_guard(
        guard,
        &ProvenanceLayout::new(repo.to_path_buf()),
        scope,
        include_retired,
    )?;
    disturbed_body(repo, base, head, rules, &graph)
}

fn disturbed_body(
    repo: &Utf8Path,
    base: String,
    head: String,
    rules: &[String],
    graph: &crate::cache::GraphEvidence,
) -> anyhow::Result<Disturbed> {
    let base_files = git::revision_files(repo, &base)?;
    let head_files = git::revision_files(repo, &head)?;
    let changes = git::changed_files(repo, &base, &head)?;
    let report = gate::report(repo, base, head, base_files, head_files, &changes, graph);
    let sites = report
        .sites
        .into_iter()
        .filter(|site| rules.is_empty() || rules.contains(&site.subject_id))
        .filter(|site| site.state != EvidenceDiffState::Untouched)
        .collect();
    Ok(Disturbed {
        base: report.base,
        head: report.head,
        files_changed: report.files_changed,
        sites,
    })
}

pub(super) fn stale(
    repo: &Utf8Path,
    scope: &ScopeId,
    request: StaleQuery,
) -> anyhow::Result<StaleResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let found = disturbed(
        repo,
        scope,
        request.base,
        request.head,
        &request.rules,
        request.include_retired,
    )?;
    let summary = summarize(&found.sites);
    let (sites, has_more) = take_page(found.sites, request.limit);
    Ok(StaleResult {
        stamp: Some(provenance_core::protocol::FreshnessStamp {
            // Stale derives every field from git ranges and the diff gate,
            // so the stamp names the whole answer unattested.
            instance: String::new(),
            serial: 0,
            digest: String::new(),
            policy: crate::operations::read_policy::RepositoryConfig::freshness_policy(
                &ProvenanceLayout::new(repo.to_path_buf()),
            ),
            attested: Vec::new(),
            live: vec![LiveConstituent::Unattested],
        }),
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
