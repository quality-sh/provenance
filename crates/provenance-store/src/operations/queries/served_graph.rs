//! Served executors for get, search, neighbors, and trace.
//!
//! Each executor answers from the stamped projection through the
//! `SqlFront` inside one held publication guard, then carries the stamp
//! the policy produced. Attested domain: graph. Live constituents: none.

use crate::layout::ProvenanceLayout;
use crate::operations::queries::trace_token::{fingerprint, ResumeToken};
use crate::operations::read_policy;
use crate::operations::traversal::{rank, SqlFront, TraversalSource};
use camino::Utf8PathBuf;
use provenance_core::protocol::{
    decode_cursor, encode_cursor, ensure_limit, ensure_max_depth, ensure_protocol_version,
    resolve_budget, take_page, AttestedDomain, GetQuery, GetResult, NeighborsQuery,
    NeighborsResult, SearchQuery, SearchResult, TraceQuery, TraceResult, TracedNode,
    VISIT_BUDGET_CAP,
};
use provenance_core::{ScopeId, StableId};
use std::collections::BTreeSet;

fn graph_attested() -> Vec<AttestedDomain> {
    vec![AttestedDomain::Graph]
}

const fn no_live() -> Vec<provenance_core::protocol::LiveConstituent> {
    Vec::new()
}

fn resolve_repo(repo: Option<Utf8PathBuf>) -> anyhow::Result<ProvenanceLayout> {
    Ok(ProvenanceLayout::new(
        crate::operations::discover_repository(repo)?,
    ))
}

/// Fetch one record by canonical id, served from the projection.
pub async fn get(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: GetQuery,
) -> anyhow::Result<GetResult> {
    let layout = resolve_repo(repo)?;
    ensure_protocol_version(request.protocol_version)?;
    let id = StableId::new(request.id)?;
    let scope = scope.clone();
    let layout_for_answer = layout.clone();
    let (mut result, stamped) = read_policy::stamped_read(
        &layout,
        graph_attested(),
        no_live(),
        move |_guard, _report| async move {
            let front = SqlFront::open(&layout_for_answer, &scope).await?;
            let node = front
                .find(request.node_type, &id, request.include_retired)
                .await?;
            front.close().await;
            Ok(GetResult {
                stamp: None,
                found: node.is_some(),
                node,
            })
        },
    )
    .await?;
    result.stamp = stamped.stamp;
    Ok(result)
}

/// Find records whose text contains a phrase, served from the projection.
pub async fn search(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: SearchQuery,
) -> anyhow::Result<SearchResult> {
    let layout = resolve_repo(repo)?;
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let needle = request.text.trim().to_lowercase();
    anyhow::ensure!(!needle.is_empty(), "search text must not be empty");
    let wanted: Vec<u8> = request.node_types.iter().map(|kind| rank(*kind)).collect();
    let scope = scope.clone();
    let include_retired = request.include_retired;
    let layout_for_answer = layout.clone();
    let (matched, stamped) = read_policy::stamped_read(
        &layout,
        graph_attested(),
        no_live(),
        move |_guard, _report| async move {
            let front = SqlFront::open(&layout_for_answer, &scope).await?;
            let matched = front
                .nodes(include_retired)
                .await?
                .into_iter()
                .filter(|node| wanted.is_empty() || wanted.contains(&rank(node.node_type())))
                .filter(|node| {
                    node.searchable_text()
                        .iter()
                        .any(|text| text.to_lowercase().contains(&needle))
                })
                .collect::<Vec<_>>();
            front.close().await;
            Ok(matched)
        },
    )
    .await?;
    let offset = request.cursor.as_deref().map(decode_cursor).transpose()?;
    let mut window = matched;
    if let Some(offset) = offset {
        window.drain(..offset.min(window.len()));
    }
    let (nodes, has_more) = take_page(window, request.limit);
    let next_cursor = has_more.then(|| encode_cursor(offset.unwrap_or(0) + request.limit));
    Ok(SearchResult {
        stamp: stamped.stamp,
        limit: request.limit,
        has_more,
        nodes,
        next_cursor,
    })
}

/// Read the records one relation away from a record, served and paged.
pub async fn neighbors(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: NeighborsQuery,
) -> anyhow::Result<NeighborsResult> {
    let layout = resolve_repo(repo)?;
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let scope = scope.clone();
    let layout_for_answer = layout.clone();
    let served_request = request.clone();
    let (mut result, stamped) = read_policy::stamped_read(
        &layout,
        graph_attested(),
        no_live(),
        move |_guard, _report| async move {
            let front = SqlFront::open(&layout_for_answer, &scope).await?;
            let mut neighbors =
                crate::operations::traversal::neighbors_raw(&front, served_request.clone()).await?;
            if let Some(cursor) = &served_request.cursor {
                let offset = decode_cursor(cursor)?;
                neighbors.drain(..offset.min(neighbors.len()));
            }
            let (paged, has_more) = take_page(neighbors, served_request.limit);
            front.close().await;
            Ok(NeighborsResult {
                stamp: None,
                id: served_request.id,
                limit: served_request.limit,
                has_more,
                neighbors: paged,
                next_cursor: None,
            })
        },
    )
    .await?;
    let offset_base = request
        .cursor
        .as_deref()
        .map(decode_cursor)
        .transpose()?
        .unwrap_or(0);
    result.stamp = stamped.stamp;
    result.next_cursor = result
        .has_more
        .then(|| encode_cursor(offset_base + request.limit));
    Ok(result)
}

/// Walk outward from a record for a bounded number of hops, with resume.
pub async fn trace(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    request: TraceQuery,
) -> anyhow::Result<TraceResult> {
    let layout = resolve_repo(repo)?;
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    ensure_max_depth(request.max_depth)?;
    let scope = scope.clone();
    let layout_for_answer = layout.clone();
    let mark = request
        .cursor
        .as_deref()
        .map(ResumeToken::decode)
        .transpose()?;
    let expected = fingerprint(
        &request.id,
        request.direction,
        &request.edge_types,
        request.max_depth,
    );
    if let Some(mark) = &mark {
        anyhow::ensure!(
            mark.matches(&expected),
            "resume token does not match the request parameters"
        );
    }
    let fingerprint = expected;
    let trace_request = request.clone();
    let config = read_policy::RepositoryConfig::load(&layout);
    let visit_budget = resolve_budget(
        request.visit_budget,
        config.read.visit_budget,
        VISIT_BUDGET_CAP,
    );
    let (result, _stamped) = read_policy::stamped_read(
        &layout,
        graph_attested(),
        no_live(),
        move |_guard, _report| async move {
            let front = SqlFront::open(&layout_for_answer, &scope).await?;
            let walked = trace_walk(
                &front,
                &trace_request,
                mark.as_ref(),
                fingerprint,
                visit_budget,
            )
            .await;
            front.close().await;
            walked
        },
    )
    .await?;
    Ok(result)
}

/// The deterministic, resumable breadth-first walk behind `trace`.
///
/// On resume, replay skips nodes at or before the watermark so no
/// `TracedNode` repeats across the page boundary; the visit budget bounds
/// how many steps the walk may expand.
async fn trace_walk<S: TraversalSource>(
    front: &S,
    request: &TraceQuery,
    mark: Option<&ResumeToken>,
    fingerprint: String,
    visit_budget: usize,
) -> anyhow::Result<TraceResult> {
    let id = StableId::new(request.id.clone())?;
    let mut seen = BTreeSet::from([id.as_str().to_string()]);
    let mut frontier = vec![id];
    let mut reached: Vec<TracedNode> = Vec::new();
    let mut expansions = 0usize;
    'walk: for depth in 1..=request.max_depth {
        let mut next = Vec::new();
        for origin in &frontier {
            for step in front
                .steps(origin, request.direction, &request.edge_types)
                .await?
            {
                expansions += 1;
                if expansions > visit_budget {
                    break 'walk;
                }
                if !seen.insert(step.id.as_str().to_string()) {
                    continue;
                }
                if let Some(node) = front
                    .find(step.node_type, &step.id, request.include_retired)
                    .await?
                {
                    next.push(node);
                }
            }
        }
        next.sort_by_key(crate::operations::traversal::node_order);
        if next.is_empty() {
            break;
        }
        frontier = next.iter().map(|node| node.id().clone()).collect();
        for node in next {
            if mark
                .as_ref()
                .is_some_and(|mark| !mark.precedes(depth, rank(node.node_type()), node.id()))
            {
                continue;
            }
            reached.push(TracedNode { depth, node });
            if reached.len() > request.limit {
                break 'walk;
            }
        }
    }
    let has_more = reached.len() > request.limit;
    let mut nodes = reached;
    let next_cursor = has_more.then(|| {
        nodes.truncate(request.limit);
        let last = nodes.last().expect("non-empty truncated page");
        ResumeToken {
            depth: last.depth,
            rank: rank(last.node.node_type()),
            id: last.node.id().as_str().to_string(),
            fingerprint,
        }
        .encode()
    });
    Ok(TraceResult {
        stamp: None,
        id: request.id.clone(),
        max_depth: request.max_depth,
        limit: request.limit,
        has_more,
        nodes,
        next_cursor,
    })
}

#[cfg(test)]
mod cursor_parity {
    use super::*;

    /// The watermark comparison the resume replay relies on: a node
    /// strictly after the watermark stays, anything before is skipped.
    #[test]
    fn resume_watermark_orders_by_depth_rank_then_id() {
        let mark = ResumeToken {
            depth: 2,
            rank: 3,
            id: "rule_b".into(),
            fingerprint: String::new(),
        };
        assert!(mark.precedes(3, 0, &StableId::new("x").unwrap()));
        assert!(mark.precedes(2, 4, &StableId::new("a").unwrap()));
        assert!(mark.precedes(2, 3, &StableId::new("rule_c").unwrap()));
        assert!(!mark.precedes(1, 9, &StableId::new("zzz").unwrap()));
        assert!(!mark.precedes(2, 3, &StableId::new("rule_a").unwrap()));
    }
}
