#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use provenance_core::NodeType;
use provenance_store::operations::catalog;
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;

fn access(repository: &Repository) -> FixtureAccess {
    FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repository.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
}

fn deny_kind(mut access: FixtureAccess, kind: NodeType) -> FixtureAccess {
    for definition in catalog::definitions().iter().filter(|definition| {
        definition.registration.queries.iter().any(|query| {
            query.name == "trace"
                && query.request.node_type == Some(kind.as_str())
        })
    }) {
        access = access.deny_operation(definition.name);
    }
    access
}

async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    arguments: Value,
) -> rmcp::model::CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new("search")
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn named_mcp_search_intersects_text_and_multiple_kinds() {
    let repository = Repository::new("The shared graph is searchable.");
    repository.all_kinds();
    let host = StatementHost::with_fixture_access(access(&repository));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    let tool = tools.iter().find(|tool| tool.name == "search").unwrap();
    assert!(tool.input_schema["properties"]["text"].is_object());
    assert!(tool.input_schema["properties"]["node_types"].is_object());
    assert!(tool.output_schema.is_some());

    let text_only = call(&client, json!({"text":"excludes code"})).await;
    assert_eq!(text_only.structured_content.as_ref().unwrap()["nodes"][0]["node_type"], "boundary");

    let kind_only = call(&client, json!({"node_types":["domain", "rule"]})).await;
    let kinds = kind_only.structured_content.as_ref().unwrap()["nodes"]
        .as_array().unwrap().iter().map(|node| node["node_type"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(kinds, ["rule", "domain"]);

    let combined = call(
        &client,
        json!({"text":"searchable", "node_types":["requirement", "rule"]}),
    )
    .await;
    let nodes = combined.structured_content.as_ref().unwrap()["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["node_type"], "rule");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_search_content_matches_the_structured_bounded_page() {
    let repository = Repository::new("The shared graph is searchable.");
    repository.all_kinds();
    let host = StatementHost::with_fixture_access(access(&repository));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let first = call(&client, json!({"text":"shared", "limit":2})).await;
    assert_ne!(first.is_error, Some(true), "{first:?}");
    let structured = first.structured_content.as_ref().unwrap();
    assert_eq!(structured["limit"], 2);
    assert_eq!(structured["has_more"], true);
    let cursor = structured["next_cursor"].as_str().unwrap();
    let readable = &first.content[0].as_text().unwrap().text;
    for node in structured["nodes"].as_array().unwrap() {
        assert!(readable.contains(node["id"].as_str().unwrap()), "{readable}");
    }
    assert!(readable.contains("limit=2"), "{readable}");
    assert!(readable.contains(cursor), "{readable}");

    let second = call(
        &client,
        json!({"text":"shared", "limit":2, "cursor":cursor}),
    )
    .await;
    let first_ids = structured["nodes"].as_array().unwrap().iter()
        .map(|node| node["id"].as_str().unwrap()).collect::<Vec<_>>();
    let second_ids = second.structured_content.as_ref().unwrap()["nodes"].as_array().unwrap().iter()
        .map(|node| node["id"].as_str().unwrap()).collect::<Vec<_>>();
    assert!(first_ids.iter().all(|id| !second_ids.contains(id)));

    let wrong_query = call(
        &client,
        json!({"text":"different", "limit":2, "cursor":cursor}),
    )
    .await;
    assert_eq!(wrong_query.is_error, Some(true), "{wrong_query:?}");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_search_applies_kind_grants_before_the_canonical_page() {
    let repository = Repository::new("The shared graph is searchable.");
    repository.all_kinds();
    let restricted = deny_kind(access(&repository), NodeType::Source);
    let host = StatementHost::with_fixture_access(restricted);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let result = call(
        &client,
        json!({"node_types":["source", "rule"], "limit":1}),
    )
    .await;
    let structured = result.structured_content.unwrap();
    assert_eq!(structured["nodes"][0]["node_type"], "rule");
    assert_eq!(structured["has_more"], false);
    assert!(structured["next_cursor"].is_null());

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_hides_search_when_no_record_kind_is_permitted() {
    let repository = Repository::new("The shared graph is searchable.");
    let mut restricted = access(&repository);
    for kind in NodeType::ALL {
        restricted = deny_kind(restricted, kind);
    }
    let host = StatementHost::with_fixture_access(restricted);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    assert!(!tools.iter().any(|tool| tool.name == "search"));

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
