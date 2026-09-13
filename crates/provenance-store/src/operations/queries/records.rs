use crate::operations::reader::ReadContext;
use provenance_core::protocol::{GetQuery, GetResult, SearchQuery, SearchResult};
use provenance_core::{NodeType, StableId};

use super::nodes;

pub(super) async fn get(ctx: &ReadContext, request: GetQuery) -> anyhow::Result<GetResult> {
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let id = StableId::new(request.id)?;
    let node = nodes::node(ctx.snapshot(), request.node_type, &id).await?;
    Ok(GetResult {
        found: node.is_some(),
        node,
    })
}

/// Search keeps canonical kind and ID order within the engine page budgets.
pub(super) async fn search(
    ctx: &ReadContext,
    request: SearchQuery,
) -> anyhow::Result<SearchResult> {
    ctx.snapshot().bound_page_work().await?;
    search_page(ctx, request)
        .await
        .map_err(crate::operations::reader::page_error)
}

async fn search_page(ctx: &ReadContext, request: SearchQuery) -> anyhow::Result<SearchResult> {
    use crate::operations::reader::{Cursor, Position};
    use crate::operations::reader::{PAGE_BYTES, RECORD_BYTES};
    use provenance_core::protocol::read_failure::ReadFailure;
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let needle = request.text.trim().to_lowercase();
    let mut wanted = if request.node_types.is_empty() {
        PROTOCOL_FIVE_DEFAULT_KINDS.to_vec()
    } else {
        request.node_types.clone()
    };
    wanted.sort_by_key(|kind| rank(*kind));
    wanted.dedup();
    let (cursor, mut position) = Cursor::open(
        ctx,
        "search",
        &(&needle, &wanted, request.limit),
        request.cursor.as_deref(),
    )?;
    let mut matched = Vec::new();
    let mut bytes = 0;
    let mut scanned = 0;
    let mut has_more = false;
    'kinds: for kind in wanted {
        if kind.rank() < position.rank {
            continue;
        }
        let mut after = if kind.rank() == position.rank {
            position.id.clone()
        } else {
            String::new()
        };
        loop {
            let count = (request.limit + 1 - matched.len()).min(512 - scanned);
            let ids = nodes::search_ids(ctx.snapshot(), kind, &after, count).await?;
            let exhausted = ids.len() < count;
            for id in ids {
                let node = nodes::page_node(ctx.snapshot(), kind, &id)
                    .await?
                    .ok_or_else(|| {
                        anyhow::anyhow!("search candidate disappeared inside snapshot")
                    })?;
                let contains_text = node
                    .searchable_text()
                    .iter()
                    .any(|text| text.to_lowercase().contains(&needle));
                if contains_text {
                    let size = serde_json::to_vec(&node)?.len();
                    if size > RECORD_BYTES {
                        return Err(ReadFailure::PageRecordTooLarge.into());
                    }
                    if matched.len() == request.limit || bytes + size > PAGE_BYTES {
                        has_more = true;
                        break 'kinds;
                    }
                    bytes += size;
                    matched.push(node);
                }
                scanned += 1;
                after.clone_from(&id);
                position = Position {
                    rank: kind.rank(),
                    id,
                    ..Position::default()
                };
                if scanned == 512 {
                    has_more = true;
                    break 'kinds;
                }
            }
            if exhausted {
                break;
            }
        }
    }
    Ok(SearchResult {
        limit: request.limit,
        has_more,
        nodes: matched,
        next_cursor: if has_more {
            Some(cursor.encode(ctx, position)?)
        } else {
            None
        },
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
