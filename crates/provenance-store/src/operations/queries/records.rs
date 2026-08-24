use provenance_core::protocol::{
    ensure_limit, ensure_protocol_version, take_page, GetQuery, GetResult, GraphNode, SearchQuery,
    SearchResult,
};
use provenance_core::{NodeType, ScopeId, StableId};
use provenance_store::state_store::StateStore;

/// Loads every canonical record a query can name, in one settled order.
///
/// Active views leave retired records out. The order is node type then
/// canonical ID, so two runs over the same state answer the same bytes.
pub(super) fn load(
    store: &StateStore,
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
    nodes.retain(|node| include_retired || !node.retired());
    nodes.sort_by(|left, right| {
        rank(left.node_type())
            .cmp(&rank(right.node_type()))
            .then_with(|| left.id().as_str().cmp(right.id().as_str()))
    });
    Ok(nodes)
}

pub(super) fn find<'a>(
    nodes: &'a [GraphNode],
    node_type: Option<NodeType>,
    id: &StableId,
) -> Option<&'a GraphNode> {
    nodes.iter().find(|node| {
        node.id() == id && node_type.is_none_or(|wanted| rank(node.node_type()) == rank(wanted))
    })
}

pub(super) fn get(
    store: &StateStore,
    scope: &ScopeId,
    request: GetQuery,
) -> anyhow::Result<GetResult> {
    ensure_protocol_version(request.protocol_version)?;
    let id = StableId::new(request.id)?;
    let nodes = load(store, scope, request.include_retired)?;
    let node = find(&nodes, Some(request.node_type), &id).cloned();
    Ok(GetResult {
        found: node.is_some(),
        node,
    })
}

pub(super) fn search(
    store: &StateStore,
    scope: &ScopeId,
    request: SearchQuery,
) -> anyhow::Result<SearchResult> {
    ensure_protocol_version(request.protocol_version)?;
    ensure_limit(request.limit)?;
    let text = request.text;
    let needle = text.trim().to_lowercase();
    anyhow::ensure!(!needle.is_empty(), "search text must not be empty");
    let wanted = request
        .node_types
        .iter()
        .map(|kind| rank(*kind))
        .collect::<Vec<_>>();
    let matched = load(store, scope, request.include_retired)?
        .into_iter()
        .filter(|node| wanted.is_empty() || wanted.contains(&rank(node.node_type())))
        .filter(|node| {
            node.searchable_text()
                .iter()
                .any(|text| text.to_lowercase().contains(&needle))
        })
        .take(request.limit + 1)
        .collect::<Vec<_>>();
    let (nodes, has_more) = take_page(matched, request.limit);
    Ok(SearchResult {
        limit: request.limit,
        has_more,
        nodes,
    })
}

/// Fixes the order node types are read in.
pub(super) const fn rank(node_type: NodeType) -> u8 {
    match node_type {
        NodeType::Source => 0,
        NodeType::Requirement => 1,
        NodeType::Resolution => 2,
        NodeType::Rule => 3,
        NodeType::Topic => 4,
        NodeType::Question => 5,
    }
}
