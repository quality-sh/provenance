use provenance_core::protocol::{
    GraphNode, QueryResponse, ResolveSymbolMatch, ResolveSymbolMatchKind, ResolveSymbolQuery,
    ResolveSymbolResult, ResolveSymbolRole, SearchQuery, SearchResult, Stamp, StampPolicy,
};
use provenance_core::{NodeType, StableId};
use provenance_macros::verifies;
use provenance_porcelain::search::{
    render_readable, PortFuture, SearchError, SearchInput, SearchOutcome, SearchPort, SymbolFuture,
};
use provenance_porcelain::Porcelain;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingPort(
    Arc<Mutex<Option<SearchQuery>>>,
    Arc<Mutex<Option<ResolveSymbolQuery>>>,
);

fn stamp() -> Stamp {
    Stamp {
        serial: 1,
        digest: "sha256:fixture".into(),
        instance_id: "fixture".into(),
        derivation: 1,
        policy: StampPolicy::AnnotateOnly,
        attested: vec!["rules".into()],
        live: Vec::new(),
    }
}

const fn outcome(response: QueryResponse<SearchResult>) -> SearchOutcome {
    SearchOutcome {
        response,
        file: None,
        symbol: None,
        matches: None,
    }
}

fn site(
    rule_id: &str,
    role: ResolveSymbolRole,
    line: Option<usize>,
    item: &str,
    method: Option<&str>,
    match_kind: ResolveSymbolMatchKind,
) -> ResolveSymbolMatch {
    ResolveSymbolMatch {
        rule_id: StableId::new(rule_id).unwrap(),
        role,
        line,
        item_name: Some(item.into()),
        verification_method: method.map(str::to_owned),
        match_kind,
    }
}

fn rule_node(id: &str, status: &str) -> GraphNode {
    node(serde_json::json!({
        "node_type": "rule", "scope_id": "default",
        "id": id, "statement": format!("Statement of {id}."), "status": status,
        "severity": "medium", "requirement_ids": ["req_parent"]
    }))
}

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
                stamp: stamp(),
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

    fn resolve_symbol(&self, request: ResolveSymbolQuery) -> SymbolFuture<'_> {
        *self.1.lock().unwrap() = Some(request.clone());
        Box::pin(async move {
            Ok(QueryResponse {
                protocol_version: provenance_core::SDK_PROTOCOL_VERSION,
                operation: "resolve-symbol",
                stamp: stamp(),
                freshness_error: None,
                freshness_cause: None,
                result: ResolveSymbolResult {
                    file: request.file.clone(),
                    symbol: request.symbol.clone(),
                    next_cursor: None,
                    limit: request.limit,
                    has_more: false,
                    rules: vec![rule_node("rule_pay", "archived")],
                    matches: vec![site(
                        "rule_pay",
                        ResolveSymbolRole::Implementation,
                        Some(1),
                        "pay",
                        None,
                        ResolveSymbolMatchKind::Symbol,
                    )],
                },
            })
        })
    }
}

#[tokio::test]
async fn filter_only_search_uses_the_canonical_query_and_result() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());
    let query = SearchInput {
        node_types: vec![NodeType::Requirement, NodeType::Rule],
        limit: Some(7),
        ..SearchInput::default()
    };

    let outcome = service.search(query).await.unwrap();

    let sent = port.0.lock().unwrap().clone().unwrap();
    assert_eq!(sent.node_types, [NodeType::Requirement, NodeType::Rule]);
    assert_eq!(sent.text, None);
    assert!(
        sent.exclude_terminal,
        "a text search omits terminal records"
    );
    assert!(port.1.lock().unwrap().is_none());
    let response = &outcome.response;
    assert_eq!(response.result.limit, 7);
    assert!(response.result.has_more);
    assert_eq!(response.result.next_cursor.as_deref(), Some("next"));
    let json = serde_json::to_value(&outcome).unwrap();
    for absent in ["file", "symbol", "matches"] {
        assert!(json.get(absent).is_none(), "{json}");
    }
}

#[tokio::test]
async fn search_refuses_a_request_without_text_or_kinds_before_the_port() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());
    let query = SearchInput::default();

    assert!(matches!(
        service.search(query).await,
        Err(SearchError::InvalidOptions { .. })
    ));
    assert!(port.0.lock().unwrap().is_none());
}

#[test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
fn readable_search_renders_an_empty_bounded_page() {
    let page = response(Vec::new(), 50, false, None, None);

    assert_eq!(
        render_readable(&outcome(page)),
        "search: 0 returned\nbounds: limit=50 has_more=false continuation=none"
    );
}

#[test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
#[verifies("rule_search_readable_lifecycle", examples)]
fn readable_search_renders_records_continuation_and_freshness() {
    let page = response(
        vec![
            node(serde_json::json!({
                "node_type": "requirement", "scope_id": "default",
                "id": "req_one", "statement": "First requirement statement.",
                "status": "active"
            })),
            node(serde_json::json!({
                "node_type": "source", "scope_id": "default",
                "id": "source_two", "name": "Source archive", "source_type": "document"
            })),
        ],
        2,
        true,
        Some("cursor-two"),
        Some("projection is stale"),
    );

    assert_eq!(
        render_readable(&outcome(page.clone())),
        concat!(
            "search: 2 returned\n",
            "- requirement req_one · active\n  First requirement statement.\n",
            "- source source_two\n  Source archive\n",
            "bounds: limit=2 has_more=true continuation=cursor-two\n",
            "warning: freshness: projection is stale"
        )
    );
    for (kind, statuses) in [
        ("requirement", "active discovery refinement resolved"),
        (
            "resolution",
            "draft review proposed approved rejected revised superseded abandoned",
        ),
        ("rule", "draft review active deprecated archived"),
        ("topic", "open explored closed archived"),
        ("question", "open blocked_on_human answered archived"),
    ] {
        for status in statuses.split_whitespace() {
            let mut record = serde_json::to_value(&page.result.nodes[0]).unwrap();
            for (key, value) in [
                ("node_type", kind),
                ("status", status),
                ("requirement_id", "req_parent"),
                ("topic_id", "topic_parent"),
                ("title", "Lifecycle record"),
                ("question", "Which state?"),
                ("position", "Use the stored state."),
                ("rationale", "The state selects current work."),
                ("resolution_method", "research"),
                ("severity", "medium"),
            ] {
                record[key] = serde_json::json!(value);
            }
            record["answer"] = serde_json::json!("Use the stored state.");
            let state_page = response(vec![node(record)], 50, false, None, None);
            let readable = render_readable(&outcome(state_page));
            assert!(
                readable.contains(&format!("- {kind} req_one · {status}\n")),
                "{readable}"
            );
        }
    }
}

#[tokio::test]
#[verifies("rule_search_file_returns_bound_rules", examples)]
async fn file_search_reads_the_code_lookup_and_returns_rules_as_nodes() {
    let port = RecordingPort::default();
    let service = Porcelain::new(port.clone());
    let input = SearchInput {
        file: Some("code.rs".into()),
        symbol: Some("pay".into()),
        line: Some(2),
        node_types: vec![NodeType::Rule],
        exclude_terminal: Some(false),
        cursor: Some("next".into()),
        ..SearchInput::default()
    };

    let outcome = service.search(input).await.unwrap();

    assert!(port.0.lock().unwrap().is_none());
    let sent = port.1.lock().unwrap().clone().unwrap();
    assert_eq!(sent.file.as_str(), "code.rs");
    assert_eq!(sent.symbol.as_deref(), Some("pay"));
    assert_eq!(sent.line, Some(2));
    assert_eq!(sent.cursor.as_deref(), Some("next"));
    assert_eq!(sent.limit, 50);
    let json = serde_json::to_value(&outcome).unwrap();
    assert_eq!(json["operation"], "resolve-symbol");
    assert_eq!(json["file"], "code.rs");
    assert_eq!(json["symbol"], "pay");
    assert_eq!(json["nodes"][0]["id"], "rule_pay");
    assert_eq!(json["matches"][0]["match_kind"], "symbol");
    assert!(json["stamp"].is_object());
}

#[tokio::test]
#[verifies("rule_search_file_refuses_other_filters", examples)]
async fn code_lookup_refusals_happen_before_the_port() {
    let file = || Some("code.rs".to_owned());
    for (input, field) in [
        (
            SearchInput {
                file: file(),
                text: Some("pay".into()),
                ..SearchInput::default()
            },
            "text",
        ),
        (
            SearchInput {
                file: file(),
                node_types: vec![NodeType::Rule, NodeType::Requirement],
                ..SearchInput::default()
            },
            "node_types",
        ),
        (
            SearchInput {
                file: file(),
                exclude_terminal: Some(true),
                ..SearchInput::default()
            },
            "exclude_terminal",
        ),
        (
            SearchInput {
                file: file(),
                limit: Some(0),
                ..SearchInput::default()
            },
            "limit",
        ),
        (
            SearchInput {
                symbol: Some("pay".into()),
                ..SearchInput::default()
            },
            "symbol",
        ),
        (
            SearchInput {
                text: Some("pay".into()),
                line: Some(3),
                ..SearchInput::default()
            },
            "line",
        ),
    ] {
        let port = RecordingPort::default();
        let service = Porcelain::new(port.clone());
        let error = service.search(input).await.unwrap_err();
        assert!(
            matches!(&error, SearchError::InvalidCodeLookup { field: refused, .. } if *refused == field),
            "{error:?}"
        );
        let message = error.to_string();
        assert!(message.starts_with(&format!("{field}: ")), "{message}");
        assert!(
            message.contains("provenance search --file <path>"),
            "{message}"
        );
        assert!(message.contains("MCP JSON: {\"file\""), "{message}");
        assert!(port.0.lock().unwrap().is_none());
        assert!(port.1.lock().unwrap().is_none());
    }
}

#[test]
#[verifies("rule_search_file_returns_bound_rules", examples)]
fn readable_code_lookup_lists_each_rule_with_its_sites() {
    let page = SearchOutcome {
        response: QueryResponse {
            protocol_version: provenance_core::SDK_PROTOCOL_VERSION,
            operation: "resolve-symbol",
            stamp: stamp(),
            freshness_error: None,
            freshness_cause: None,
            result: SearchResult {
                next_cursor: None,
                limit: 50,
                has_more: false,
                nodes: vec![
                    rule_node("rule_pay", "active"),
                    rule_node("rule_old", "archived"),
                ],
            },
        },
        file: Some("code.rs".into()),
        symbol: Some("pay".into()),
        matches: Some(vec![
            site(
                "rule_pay",
                ResolveSymbolRole::Implementation,
                Some(1),
                "pay",
                None,
                ResolveSymbolMatchKind::Symbol,
            ),
            site(
                "rule_pay",
                ResolveSymbolRole::Verification,
                Some(3),
                "checks_pay",
                Some("examples"),
                ResolveSymbolMatchKind::File,
            ),
            site(
                "rule_old",
                ResolveSymbolRole::Verification,
                None,
                "old_check",
                Some("tests"),
                ResolveSymbolMatchKind::File,
            ),
        ]),
    };

    assert_eq!(
        render_readable(&page),
        concat!(
            "search: 2 returned for code.rs symbol pay\n",
            "- rule rule_pay · active\n  Statement of rule_pay.\n",
            "  · implementation line 1 pay (symbol match)\n",
            "  · verification line 3 checks_pay examples (file match)\n",
            "- rule rule_old · archived\n  Statement of rule_old.\n",
            "  · verification typed binding old_check tests (file match)\n",
            "bounds: limit=50 has_more=false continuation=none"
        )
    );
}
