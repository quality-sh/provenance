//! Served executors whose answers mix projected and live state.
//!
//! `impact` and `resolve_symbol` union projection bindings with scanner
//! sites from the working tree; the scan is bounded by the scan budget.
//! `evidence` reads bindings and reviews from the projection, keeps
//! verification runs on cache JSONL, and reports per-collection paging.
//! `stale` stays git machinery: the stamp names the whole answer
//! unattested.

use crate::layout::ProvenanceLayout;
use crate::operations::read_policy;
use crate::operations::sites;
use crate::operations::traversal::{SqlFront, TraversalSource};
use camino::Utf8PathBuf;
use provenance_core::protocol::{
    decode_cursor, encode_cursor, ensure_limit, ensure_protocol_version, resolve_budget, take_page,
    AttestedDomain, CollectionPage, Direction, ImpactQuery, ImpactResult, LiveConstituent,
    ResolveSymbolQuery, ResolveSymbolResult, StaleEvidence, StaleQuery, StaleResult,
    VISIT_BUDGET_CAP,
};
use provenance_core::{
    ImplementationBinding, NodeType, RequirementReview, ScopeId, StableId, VerificationBinding,
};
use std::collections::BTreeSet;

fn scan_budgeted(query_budget: Option<usize>, default: usize) -> usize {
    query_budget
        .unwrap_or(default)
        .min(provenance_core::protocol::SCAN_BUDGET_CAP)
}

fn graph_and_bindings() -> Vec<AttestedDomain> {
    vec![AttestedDomain::Graph, AttestedDomain::Bindings]
}

/// Read the Rules a record reaches, with the code standing behind them.
pub async fn impact(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: ImpactQuery,
) -> anyhow::Result<ImpactResult> {
    let repo_path = crate::operations::discover_repository(repo)?;
    let layout = ProvenanceLayout::new(repo_path.clone());
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let scope = scope.clone();
    let scan_budget = scan_budgeted(
        request.scan_budget,
        read_policy::RepositoryConfig::load(&layout)
            .read
            .scan_budget,
    );
    let layout_for_answer = layout.clone();
    let impact_request = request.clone();
    let (result, stamped) = read_policy::stamped_read(
        &layout,
        graph_and_bindings(),
        vec![LiveConstituent::ScannerSites],
        move |_guard, _report| async move {
            let front = SqlFront::open(&layout_for_answer, &scope).await?;
            let walked = impact_walk(&front, &impact_request).await?;
            let (wanted, has_more) = page_strings(
                walked,
                impact_request.cursor.as_deref(),
                impact_request.limit,
            )?;
            let scans = provenance_scanner::scan_path(&repo_path)?;
            let implementations: Vec<ImplementationBinding> = front
                .payloads("implementation_bindings")
                .await?
                .iter()
                .map(|payload| serde_json::from_str(payload))
                .collect::<Result<_, _>>()?;
            let verifications: Vec<VerificationBinding> = front
                .payloads("verification_bindings")
                .await?
                .iter()
                .map(|payload| serde_json::from_str(payload))
                .collect::<Result<_, _>>()?;
            let evidence = sites::Evidence {
                scans: &scans[..scans.len().min(scan_budget)],
                verifications: &verifications,
                implementations: &implementations,
            };
            let mut affected_rules = Vec::new();
            for rule in wanted {
                affected_rules.push(evidence.affected_rule(&repo_path, StableId::new(rule)?));
            }
            front.close().await;
            Ok((affected_rules, has_more))
        },
    )
    .await?;
    let (affected_rules, has_more) = result;
    let offset_base = request
        .cursor
        .as_deref()
        .map(decode_cursor)
        .transpose()?
        .unwrap_or(0);
    Ok(ImpactResult {
        stamp: stamped.stamp,
        id: request.id,
        limit: request.limit,
        has_more,
        affected_rules,
        next_cursor: has_more.then(|| encode_cursor(offset_base + request.limit)),
    })
}

/// The depth-capped, visit-budgeted walk collecting reached Rules.
async fn impact_walk<S: TraversalSource>(
    front: &S,
    request: &ImpactQuery,
) -> anyhow::Result<BTreeSet<String>> {
    let id = StableId::new(request.id.clone())?;
    let mut seen = BTreeSet::from([id.as_str().to_string()]);
    let mut rules = BTreeSet::new();
    if front
        .find(NodeType::Rule, &id, request.include_retired)
        .await?
        .is_some()
    {
        rules.insert(id.as_str().to_string());
    }
    let mut frontier = vec![id];
    let mut expansions = 0usize;
    let visit_budget = resolve_budget(
        request.visit_budget,
        provenance_core::protocol::VISIT_BUDGET_DEFAULT,
        VISIT_BUDGET_CAP,
    );
    'walk: for _ in 0..provenance_core::protocol::TRACE_MAX_DEPTH {
        let mut next = Vec::new();
        for origin in &frontier {
            for step in front.steps(origin, Direction::Out, &[]).await? {
                expansions += 1;
                if expansions > visit_budget {
                    break 'walk;
                }
                if !seen.insert(step.id.as_str().to_string()) {
                    continue;
                }
                if front
                    .find(step.node_type, &step.id, request.include_retired)
                    .await?
                    .is_none()
                {
                    continue;
                }
                if step.node_type == NodeType::Rule {
                    rules.insert(step.id.as_str().to_string());
                }
                next.push(step.id);
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    Ok(rules)
}

/// Applies an offset cursor and the page limit to one ordered collection.
fn page_strings(
    items: std::collections::BTreeSet<String>,
    cursor: Option<&str>,
    limit: usize,
) -> anyhow::Result<(Vec<String>, bool)> {
    let mut items: Vec<String> = items.into_iter().collect();
    if let Some(cursor) = cursor {
        let offset = decode_cursor(cursor)?;
        items.drain(..offset.min(items.len()));
    }
    Ok(take_page(items, limit))
}

/// Read the Rules bound to one code site, hybrid and honest.
pub async fn resolve_symbol(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: ResolveSymbolQuery,
) -> anyhow::Result<ResolveSymbolResult> {
    let repo_path = crate::operations::discover_repository(repo)?;
    let layout = ProvenanceLayout::new(repo_path.clone());
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let scope = scope.clone();
    let include_retired = request.include_retired;
    let scan_budget = scan_budgeted(
        request.scan_budget,
        read_policy::RepositoryConfig::load(&layout)
            .read
            .scan_budget,
    );
    let layout_for_answer = layout.clone();
    let file = request.file.clone();
    let symbol = request.symbol.clone();
    let line = request.line;
    let (result, stamped) = read_policy::stamped_read(
        &layout,
        graph_and_bindings(),
        vec![LiveConstituent::ScannerSites],
        move |_guard, _report| async move {
            let front = SqlFront::open(&layout_for_answer, &scope).await?;
            let symbol = symbol.as_deref();
            let implementations: Vec<ImplementationBinding> = front
                .payloads("implementation_bindings")
                .await?
                .iter()
                .map(|payload| serde_json::from_str(payload))
                .collect::<Result<_, _>>()?;
            let verifications: Vec<VerificationBinding> = front
                .payloads("verification_bindings")
                .await?
                .iter()
                .map(|payload| serde_json::from_str(payload))
                .collect::<Result<_, _>>()?;
            let scans = provenance_scanner::scan_path(&repo_path)?;
            let mut ids = BTreeSet::new();
            for site in provenance_scanner::source_sites(&scans[..scans.len().min(scan_budget)]) {
                if crate::operations::sites::relative(&repo_path, site.file_path()) == *file
                    && symbol.is_none_or(|wanted| site.item_name() == Some(wanted))
                    && line.is_none_or(|wanted| site.line() == wanted)
                {
                    ids.insert(site.rule_id().to_string());
                }
            }
            if line.is_none() {
                for binding in &implementations {
                    if binding.file == *file && symbol.is_none_or(|wanted| binding.symbol == wanted)
                    {
                        ids.insert(binding.rule_id.as_str().to_string());
                    }
                }
                for binding in &verifications {
                    if binding.file == *file
                        && symbol.is_none_or(|wanted| binding.symbol.as_deref() == Some(wanted))
                    {
                        ids.insert(binding.rule_id.as_str().to_string());
                    }
                }
            }
            let mut matched = Vec::new();
            for id in ids {
                if let Ok(id) = StableId::new(id) {
                    if let Some(node) = front.find(NodeType::Rule, &id, include_retired).await? {
                        matched.push(node);
                    }
                }
            }
            matched.sort_by_key(crate::operations::traversal::node_order);
            front.close().await;
            let (rules, has_more) = take_page(matched, request.limit);
            Ok((rules, has_more))
        },
    )
    .await?;
    let (rules, has_more) = result;
    Ok(ResolveSymbolResult {
        stamp: stamped.stamp,
        file: request.file,
        symbol: request.symbol,
        limit: request.limit,
        has_more,
        rules,
    })
}

/// Read everything standing behind one Rule, per-collection paged.
pub async fn evidence(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: provenance_core::protocol::EvidenceQuery,
) -> anyhow::Result<provenance_core::protocol::EvidenceResult> {
    let repo_path = crate::operations::discover_repository(repo)?;
    let layout = ProvenanceLayout::new(repo_path.clone());
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let rule = StableId::new(request.rule.clone())?;
    let scope = scope.clone();
    let include_retired = request.include_retired;
    let limit = request.limit;
    let layout_for_answer = layout.clone();
    let (mut result, stamped) = read_policy::stamped_read(
        &layout,
        vec![AttestedDomain::Bindings, AttestedDomain::Reviews],
        vec![
            LiveConstituent::VerificationRuns,
            LiveConstituent::StaleDiff,
        ],
        move |guard, _report| async move {
            let front = SqlFront::open(&layout_for_answer, &scope).await?;
            let mut answer = evidence_answer(
                &front,
                &layout_for_answer,
                &scope,
                &rule,
                include_retired,
                limit,
            )
            .await?;
            let stale = request
                .base
                .clone()
                .map(|base| {
                    crate::operations::queries::stale::disturbed_under_guard(
                        &guard,
                        &repo_path,
                        &scope,
                        base,
                        request.head.clone(),
                        std::slice::from_ref(&request.rule),
                        include_retired,
                    )
                    .map(|found| StaleEvidence {
                        base: found.base,
                        head: found.head,
                        sites: found.sites,
                    })
                })
                .transpose()?;
            answer.stale = stale;
            front.close().await;
            Ok(answer)
        },
    )
    .await?;
    result.stamp = stamped.stamp;
    Ok(result)
}

/// Loads the projected halves of one Rule's evidence and pages every
/// collection truthfully.
async fn evidence_answer(
    front: &SqlFront,
    layout: &ProvenanceLayout,
    scope: &ScopeId,
    rule: &StableId,
    include_retired: bool,
    limit: usize,
) -> anyhow::Result<provenance_core::protocol::EvidenceResult> {
    let mut implementations: Vec<ImplementationBinding> = front
        .payloads("implementation_bindings")
        .await?
        .iter()
        .map(|payload| serde_json::from_str(payload))
        .collect::<Result<_, _>>()?;
    implementations
        .retain(|binding| binding.rule_id == *rule && (include_retired || !binding.retired));
    implementations.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    let mut verifications: Vec<VerificationBinding> = front
        .payloads("verification_bindings")
        .await?
        .iter()
        .map(|payload| serde_json::from_str(payload))
        .collect::<Result<_, _>>()?;
    verifications
        .retain(|binding| binding.rule_id == *rule && (include_retired || !binding.retired));
    verifications.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    let mut reviews: Vec<RequirementReview> = front
        .payloads("requirement_reviews")
        .await?
        .iter()
        .map(|payload| serde_json::from_str(payload))
        .collect::<Result<_, _>>()?;
    reviews.retain(|review| review.rule_id == *rule);
    reviews.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    let review_required = !reviews.is_empty();

    let store = crate::state_store::StateStore::new(layout.clone());
    let mut runs: Vec<_> = store
        .list_verification_runs(scope)?
        .into_iter()
        .filter(|run| run.rule_id == *rule)
        .collect();
    runs.sort_by(|left, right| {
        right
            .started_at
            .cmp(&left.started_at)
            .then_with(|| right.id.as_str().cmp(left.id.as_str()))
    });
    let latest_verification_run = runs.first().cloned();
    runs.truncate(limit + 1);

    let (implementation_bindings, cut_implementations) = take_page(implementations, limit);
    let (verification_bindings, cut_verifications) = take_page(verifications, limit);
    let (verification_runs, cut_runs) = take_page(runs, limit);
    let (paged_reviews, cut_reviews) = take_page(reviews, limit);

    let page = |has_more: bool, offset: usize| CollectionPage {
        has_more,
        next_cursor: has_more.then(|| encode_cursor(offset)),
    };
    Ok(provenance_core::protocol::EvidenceResult {
        stamp: None,
        rule_id: rule.as_str().to_string(),
        limit,
        has_more: cut_implementations || cut_verifications || cut_runs || cut_reviews,
        implementation_bindings_page: page(cut_implementations, limit),
        verification_bindings_page: page(cut_verifications, limit),
        verification_runs_page: page(cut_runs, limit),
        reviews_page: page(cut_reviews, limit),
        implementation_bindings,
        verification_bindings,
        verification_runs,
        latest_verification_run,
        review_required,
        reviews: paged_reviews,
        stale: None,
    })
}

/// Read which evidence sites a commit range disturbed; git machinery only.
pub fn stale(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: StaleQuery,
) -> anyhow::Result<StaleResult> {
    let repo_path = crate::operations::discover_repository(repo)?;
    crate::operations::queries::stale::stale(&repo_path, scope, request)
}
