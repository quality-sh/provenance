use provenance_core::protocol::{
    GraphNode, QueryResponse, SearchQuery, SearchResult, Stamp, StampPolicy,
};
use provenance_core::NodeType;
use provenance_porcelain::search::{PortFuture, SearchError, SearchPort};
use provenance_porcelain::Porcelain;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingPort(Arc<Mutex<Option<SearchQuery>>>);

impl SearchPort for RecordingPort {
    fn search(&self, request: SearchQuery) -> PortFuture<'_> {
        *self.0.lock().unwrap() = Some(request.clone());
        Box::pin(async move {
            Ok(QueryResponse {
                protocol_version: provenance_core::SDK_PROTOCOL_VERSION,
                operation: "search",
                stamp: Stamp {
                    serial: 1,
                    digest: "sha256:fixture".into(),
                    instance_id: "fixture".into(),
                    derivation: 1,
                    policy: StampPolicy::AnnotateOnly,
                    attested: vec!["rules".into()],
                    live: Vec::new(),
                },
                freshness_error: None,
                freshness_cause: None,
                result: SearchResult {
                    next_cursor: Some("next".into()),
                    limit: request.limit,
                    has_more: true,
                    nodes: Vec::<GraphNode>::new(),
                },
            })
        })
    }
}

#[tokio::test]
async fn filter_only_search_uses_the_canonical_query_and_result() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());
    let query = SearchQuery {
        protocol_version: None,
        cursor: None,
        text: None,
        node_types: vec![NodeType::Requirement, NodeType::Rule],
        limit: 7,
    };

    let response = service.search(query).await.unwrap();

    let sent = port.0.lock().unwrap().clone().unwrap();
    assert_eq!(sent.node_types, [NodeType::Requirement, NodeType::Rule]);
    assert_eq!(sent.text, None);
    assert_eq!(response.result.limit, 7);
    assert!(response.result.has_more);
    assert_eq!(response.result.next_cursor.as_deref(), Some("next"));
}

#[tokio::test]
async fn search_refuses_a_request_without_text_or_kinds_before_the_port() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());
    let query = SearchQuery {
        protocol_version: None,
        cursor: None,
        text: None,
        node_types: Vec::new(),
        limit: 50,
    };

    assert!(matches!(
        service.search(query).await,
        Err(SearchError::InvalidOptions)
    ));
    assert!(port.0.lock().unwrap().is_none());
}
