use provenance_core::protocol::{
    GraphNode, QueryResponse, SearchQuery, SearchResult, Stamp, StampPolicy,
};
use provenance_core::NodeType;
use provenance_porcelain::search::{render_readable, PortFuture, SearchError, SearchPort};
use provenance_porcelain::Porcelain;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingPort(Arc<Mutex<Option<SearchQuery>>>);

fn response(
    nodes: Vec<GraphNode>,
    limit: usize,
    has_more: bool,
    next_cursor: Option<&str>,
    freshness_error: Option<&str>,
) -> QueryResponse<SearchResult> {
    QueryResponse {
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
        freshness_error: freshness_error.map(str::to_owned),
        freshness_cause: None,
        result: SearchResult {
            next_cursor: next_cursor.map(str::to_owned),
            limit,
            has_more,
            nodes,
        },
    }
}

fn node(value: serde_json::Value) -> GraphNode {
    serde_json::from_value(value).unwrap()
}

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

#[test]
fn readable_search_renders_an_empty_bounded_page() {
    let page = response(Vec::new(), 50, false, None, None);

    assert_eq!(
        render_readable(&page),
        "search: 0 returned\nbounds: limit=50 has_more=false continuation=none"
    );
}

#[test]
fn readable_search_renders_records_continuation_and_freshness() {
    let page = response(
        vec![
            node(serde_json::json!({
                "node_type": "requirement", "schema_version": 2, "scope_id": "default",
                "id": "req_one", "statement": "First requirement statement.",
                "status": "active"
            })),
            node(serde_json::json!({
                "node_type": "source", "schema_version": 2, "scope_id": "default",
                "id": "source_two", "name": "Source archive", "source_type": "document"
            })),
        ],
        2,
        true,
        Some("cursor-two"),
        Some("projection is stale"),
    );

    assert_eq!(
        render_readable(&page),
        concat!(
            "search: 2 returned\n",
            "- requirement req_one\n  First requirement statement.\n",
            "- source source_two\n  Source archive\n",
            "bounds: limit=2 has_more=true continuation=cursor-two\n",
            "warning: freshness: projection is stale"
        )
    );
}
