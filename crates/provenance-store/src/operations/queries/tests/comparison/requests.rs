//! The request set the comparison tests runs over one store, derived from the
//! records the store holds so every kind and every operation gets a case.

use super::test_stores::TestStore;
use crate::operations::queries::bindings::Bindings;
use crate::operations::queries::tests::baseline::records;
use provenance_core::protocol::{
    Direction, EvidenceQuery, GetQuery, GraphNode, ImpactQuery, NeighborsQuery, ResolveSymbolQuery,
    SearchQuery, StaleQuery, TraceQuery, SDK_PROTOCOL_VERSION,
};
use provenance_core::NodeType;

/// How many records of each kind the set names.
const PER_KIND: usize = 3;

#[derive(Debug, Clone)]
pub enum Request {
    Get(GetQuery),
    Search(SearchQuery),
    Neighbors(NeighborsQuery),
    Trace(TraceQuery),
    Impact(ImpactQuery),
    Evidence(EvidenceQuery),
    Stale(StaleQuery),
    ResolveSymbol(ResolveSymbolQuery),
}

impl Request {
    pub fn operation(&self) -> &'static str {
        match self {
            Self::Get(_) => "get",
            Self::Search(_) => "search",
            Self::Neighbors(_) => "neighbors",
            Self::Trace(_) => "trace",
            Self::Impact(_) => "impact",
            Self::Evidence(_) => "evidence",
            Self::Stale(_) => "stale",
            Self::ResolveSymbol(_) => "resolve_symbol",
        }
    }

    /// One short label for a printed row.
    pub fn describe(&self) -> String {
        match self {
            Self::Get(query) => format!("{}:{}", kind_word(query.node_type), query.id),
            Self::Search(query) => format!("{:?}/{}", query.text, query.node_types.len()),
            Self::Neighbors(query) => format!("{}/limit={}", query.id, query.limit),
            Self::Trace(query) => format!("{}/limit={}", query.id, query.limit),
            Self::Impact(query) => query.id.clone(),
            Self::Evidence(query) => {
                format!("{}/base={}", query.rule, query.base.is_some())
            }
            Self::Stale(query) => format!("base={}", &query.base[..7.min(query.base.len())]),
            Self::ResolveSymbol(query) => query.file.to_string(),
        }
    }
}

fn kind_word(kind: NodeType) -> String {
    serde_json::to_value(kind)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

/// Up to `PER_KIND` records of each kind, in the served order, retired
/// ones included.
fn sampled(nodes: &[GraphNode]) -> Vec<&GraphNode> {
    let mut picked = Vec::new();
    for kind in [
        NodeType::Source,
        NodeType::Requirement,
        NodeType::Resolution,
        NodeType::Rule,
        NodeType::Topic,
        NodeType::Question,
        NodeType::Domain,
        NodeType::Boundary,
    ] {
        picked.extend(
            nodes
                .iter()
                .filter(|node| node.node_type() == kind)
                .take(PER_KIND),
        );
    }
    picked
}

pub fn for_store(store: &TestStore) -> Vec<Request> {
    let state = store.state_store();
    let nodes = records::load(&state, &store.scope, true).unwrap();
    let sample = sampled(&nodes);
    let mut requests = Vec::new();
    for node in &sample {
        let id = node.id().as_str().to_string();
        requests.push(Request::Get(GetQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            node_type: node.node_type(),
            id: id.clone(),
            include_retired: false,
        }));
        if node.retired() {
            requests.push(Request::Get(GetQuery {
                protocol_version: Some(SDK_PROTOCOL_VERSION),
                node_type: node.node_type(),
                id: id.clone(),
                include_retired: true,
            }));
        }
        requests.push(Request::Neighbors(neighbors(&id, node.retired(), 50)));
        requests.push(Request::Trace(trace(&id, node.retired(), 50)));
        requests.push(Request::Impact(ImpactQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            id: id.clone(),
            node_type: None,
            include_retired: node.retired(),
            limit: 50,
        }));
        if node.node_type() == NodeType::Rule {
            requests.push(Request::Evidence(evidence(&id, None)));
            if let Some(base) = &store.base_commit {
                requests.push(Request::Evidence(evidence(&id, Some(base.clone()))));
            }
        }
    }
    if let Some(first) = sample.first() {
        let id = first.id().as_str().to_string();
        requests.push(Request::Neighbors(neighbors(&id, false, 1)));
        requests.push(Request::Trace(trace(&id, false, 2)));
    }
    for needle in ["pay", "over", "e"] {
        requests.push(Request::Search(search(needle, Vec::new())));
    }
    requests.push(Request::Search(search(
        "e",
        vec![NodeType::Domain, NodeType::Boundary, NodeType::Rule],
    )));
    if let Some(base) = &store.base_commit {
        requests.push(Request::Stale(StaleQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            base: base.clone(),
            head: None,
            rules: Vec::new(),
            include_retired: false,
            limit: 50,
        }));
    }
    let bindings = Bindings::load(&state, &store.scope, true).unwrap();
    let mut files: Vec<String> = bindings
        .implementations
        .iter()
        .map(|binding| binding.file.to_string())
        .chain(
            bindings
                .verifications
                .iter()
                .map(|binding| binding.file.to_string()),
        )
        .collect();
    files.push("src/pay.rs".into());
    files.sort();
    files.dedup();
    for file in files.into_iter().take(PER_KIND) {
        requests.push(Request::ResolveSymbol(ResolveSymbolQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            file: file.into(),
            symbol: None,
            line: None,
            include_retired: false,
            limit: 50,
        }));
    }
    requests
}

pub fn neighbors(id: &str, include_retired: bool, limit: usize) -> NeighborsQuery {
    NeighborsQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: id.to_string(),
        node_type: None,
        direction: Direction::Both,
        relations: Vec::new(),
        include_retired,
        limit,
    }
}

pub fn trace(id: &str, include_retired: bool, limit: usize) -> TraceQuery {
    TraceQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: id.to_string(),
        node_type: None,
        direction: Direction::Both,
        relations: Vec::new(),
        max_depth: 3,
        include_retired,
        limit,
    }
}

pub fn evidence(rule: &str, base: Option<String>) -> EvidenceQuery {
    EvidenceQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        rule: rule.to_string(),
        base,
        head: None,
        include_retired: false,
        limit: 50,
    }
}

pub fn search(text: &str, node_types: Vec<NodeType>) -> SearchQuery {
    SearchQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        text: text.to_string(),
        node_types,
        include_retired: false,
        limit: 10,
    }
}
