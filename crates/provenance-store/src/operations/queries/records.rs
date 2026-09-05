use crate::operations::reader::ReadContext;
#[cfg(test)]
use provenance_core::protocol::GraphNode;
use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, GetQuery, GetResult, SearchQuery,
    SearchResult,
};
#[cfg(test)]
use provenance_core::ScopeId;
use provenance_core::{NodeType, StableId};

use super::nodes;

/// Loads every canonical record a query can name, in one settled order.
///
/// Active views leave retired records out. The order is node type then
/// canonical ID, so two runs over the same state answer the same bytes.
/// No operation reads canonical records any more; the comparison tests
/// still do, until the baseline goes.
#[cfg(test)]
pub(super) fn load(
    store: &crate::state_store::StateStore,
    scope: &ScopeId,
    include_retired: bool,
) -> anyhow::Result<Vec<GraphNode>> {
    let mut nodes = Vec::new();
    nodes.extend(
        store
            .list_sources(scope)?
            .into_iter()
            .map(|record| GraphNode::Source(Box::new(record))),
    );
    nodes.extend(
        store
            .list_requirements(scope)?
            .into_iter()
            .map(|record| GraphNode::Requirement(Box::new(record))),
    );
    nodes.extend(
        store
            .list_resolutions(scope)?
            .into_iter()
            .map(|record| GraphNode::Resolution(Box::new(record))),
    );
    nodes.extend(
        store
            .list_rules(scope)?
            .into_iter()
            .map(|record| GraphNode::Rule(Box::new(record))),
    );
    nodes.extend(
        store
            .list_topics(scope)?
            .into_iter()
            .map(|record| GraphNode::Topic(Box::new(record))),
    );
    nodes.extend(
        store
            .list_questions(scope)?
            .into_iter()
            .map(|record| GraphNode::Question(Box::new(record))),
    );
    nodes.extend(
        store
            .list_domains(scope)?
            .into_iter()
            .map(|record| GraphNode::Domain(Box::new(record))),
    );
    nodes.extend(
        store
            .list_boundaries(scope)?
            .into_iter()
            .map(|record| GraphNode::Boundary(Box::new(record))),
    );
    nodes.retain(|node| include_retired || !node.retired());
    nodes.sort_by(|left, right| {
        rank(left.node_type())
            .cmp(&rank(right.node_type()))
            .then_with(|| left.id().as_str().cmp(right.id().as_str()))
    });
    Ok(nodes)
}

pub(super) async fn get(ctx: &ReadContext, request: GetQuery) -> anyhow::Result<GetResult> {
    ensure_protocol_version(request.protocol_version)?;
    let id = StableId::new(request.id)?;
    let node = nodes::node(
        ctx.snapshot(),
        request.node_type,
        &id,
        request.include_retired,
    )
    .await?;
    Ok(GetResult {
        found: node.is_some(),
        node,
    })
}

/// Visits the wanted kinds in rank order, each table once, and stops
/// reading once the page and its cut flag are decided. The table's
/// `instr` match is over the joined pieces, so a needle spanning two
/// pieces can come back; the per-piece `contains` decides.
pub(super) async fn search(
    ctx: &ReadContext,
    request: SearchQuery,
) -> anyhow::Result<SearchResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let text = request.text;
    let needle = text.trim().to_lowercase();
    anyhow::ensure!(!needle.is_empty(), "search text must not be empty");
    // Protocol version 5 compatibility: a request that names no kinds gets
    // the six kinds version 5 always answered. Domains and boundaries are
    // opt-in through an explicit node_types entry, so a strict old client
    // never meets a kind it cannot read.
    let mut wanted = if request.node_types.is_empty() {
        PROTOCOL_FIVE_DEFAULT_KINDS.to_vec()
    } else {
        request.node_types.clone()
    };
    wanted.sort_by_key(|kind| rank(*kind));
    wanted.dedup_by_key(|kind| rank(*kind));
    let mut matched = Vec::new();
    for kind in wanted {
        if matched.len() > request.limit {
            break;
        }
        let room = request.limit + 1 - matched.len();
        let rows = nodes::search(ctx.snapshot(), kind, &needle, request.include_retired).await?;
        matched.extend(
            rows.into_iter()
                .filter(|node| {
                    node.searchable_text()
                        .iter()
                        .any(|text| text.to_lowercase().contains(&needle))
                })
                .take(room),
        );
    }
    let (nodes, has_more) = take_page(matched, request.limit);
    Ok(SearchResult {
        limit: request.limit,
        has_more,
        nodes,
    })
}

/// The kinds a version-5 search answers when the request names none.
const PROTOCOL_FIVE_DEFAULT_KINDS: [NodeType; 6] = [
    NodeType::Source,
    NodeType::Requirement,
    NodeType::Resolution,
    NodeType::Rule,
    NodeType::Topic,
    NodeType::Question,
];

/// Fixes the order node types are read in: the one contract rank on
/// `NodeType`, so the served order and the traversal order cannot drift.
pub(super) const fn rank(node_type: NodeType) -> u8 {
    node_type.rank()
}
