use crate::operations::reader::ReadContext;
use provenance_core::protocol::{take_page, GetQuery, GetResult, SearchQuery, SearchResult};
use provenance_core::{NodeType, StableId};

use super::nodes;

pub(super) async fn get(ctx: &ReadContext, request: GetQuery) -> anyhow::Result<GetResult> {
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
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
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let text = request.text;
    let needle = text.trim().to_lowercase();
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
