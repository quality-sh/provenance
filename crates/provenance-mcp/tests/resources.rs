#![cfg(feature = "test-fixture")]
#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
    #[path = "../../../provenance-transport/tests/support/resource_http.rs"]
    pub mod resource_http;
}
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, host};

#[tokio::test]
async fn graph_queries_apply_declared_relation_filters() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, false);

    for query in ["neighbors", "trace"] {
        let (status, filtered) = call(
            &host,
            "GET",
            &format!("/requirements/req_shared?query={query}&relations=requirement_ids&limit=20"),
            None,
        )
        .await;
        assert_eq!(status, 200, "{filtered}");
        let items = if query == "neighbors" {
            filtered["data"]["neighbors"].as_array().unwrap()
        } else {
            filtered["data"]["nodes"].as_array().unwrap()
        };
        assert!(!items.is_empty(), "{query}: {filtered}");
        if query == "neighbors" {
            assert!(
                items
                    .iter()
                    .all(|item| item["relation"] == "requirement_ids"),
                "{query}: {filtered}"
            );
        } else {
            assert!(
                items.iter().all(|item| {
                    item["node"]["id"] == "rule_shared" || item["node"]["id"] == "resolution_shared"
                }),
                "{query}: {filtered}"
            );
        }
    }

    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server =
        tokio::spawn(async move { provenance_mcp::serve(server_host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let result = client
        .call_tool(
            CallToolRequestParams::new("api").with_arguments(
                json!({
                    "path":"requirements/req_shared",
                    "query":{"query":"neighbors", "relations":"requirement_ids", "limit":"20"}
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    let value = result.structured_content.unwrap();
    assert!(value["data"]["neighbors"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["relation"] == "requirement_ids"));
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_keeps_role_subsets_and_returns_the_http_envelope() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server =
        tokio::spawn(async move { provenance_mcp::serve(server_host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert!(!tools.iter().any(|tool| tool.name == "list-sources"));
    assert!(tools
        .iter()
        .all(|tool| tool.description.as_deref() != Some("Invoke the shared operation.")));
    let result = client
        .call_tool(
            CallToolRequestParams::new("api")
                .with_arguments(json!({"path":"sources"}).as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let value = result.structured_content.unwrap();
    assert!(value["data"]["items"].is_array(), "{value}");
    assert!(value["meta"].is_object());

    let filter_only_arguments: serde_json::Map<String, Value> =
        json!({"path":"sources", "query":{"query":"search"}})
            .as_object()
            .unwrap()
            .clone();
    let filter_only: rmcp::model::CallToolResult = client
        .call_tool(CallToolRequestParams::new("api").with_arguments(filter_only_arguments))
        .await
        .unwrap();
    assert_ne!(filter_only.is_error, Some(true), "{filter_only:?}");
    let filter_only_value = filter_only.structured_content.unwrap();
    assert!(filter_only_value["data"]["items"].is_array());

    let search_arguments: serde_json::Map<String, Value> = json!({
        "path":"sources", "query":{"query":"search", "text":"shared"}
    })
    .as_object()
    .unwrap()
    .clone();
    let search: rmcp::model::CallToolResult = client
        .call_tool(CallToolRequestParams::new("api").with_arguments(search_arguments))
        .await
        .unwrap();
    assert_ne!(search.is_error, Some(true), "{search:?}");

    let extra_arguments: serde_json::Map<String, Value> = json!({
        "path":"sources", "query":{"query":"search", "text":"shared", "base":"main"}
    })
    .as_object()
    .unwrap()
    .clone();
    let extra: rmcp::model::CallToolResult = client
        .call_tool(CallToolRequestParams::new("api").with_arguments(extra_arguments))
        .await
        .unwrap();
    assert_eq!(extra.is_error, Some(true), "{extra:?}");
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
