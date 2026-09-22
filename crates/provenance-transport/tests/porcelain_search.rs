#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use provenance_store::operations::catalog::{self, Operation as _};
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

fn deny_operations(mut access: FixtureAccess, operations: &[&str]) -> FixtureAccess {
    for operation in operations {
        access = access.deny_operation(operation);
    }
    access
}

const LIST_SEARCH_OPERATIONS: [&str; 8] = [
    "list-sources",
    "list-requirements",
    "list-resolutions",
    "list-rules",
    "list-topics",
    "list-questions",
    "list-domains",
    "list-boundaries",
];

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

async fn assert_search_hidden_and_refused(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
) {
    let tools = client.list_all_tools().await.unwrap();
    assert!(!tools.iter().any(|tool| tool.name == "search"));
    let direct = client.call_tool(CallToolRequestParams::new("search")).await;
    assert!(direct.is_err(), "{direct:?}");
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
    assert_eq!(
        text_only.structured_content.as_ref().unwrap()["nodes"][0]["node_type"],
        "boundary"
    );

    let kind_only = call(&client, json!({"node_types":["domain", "rule"]})).await;
    let kinds = kind_only.structured_content.as_ref().unwrap()["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["node_type"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(kinds, ["rule", "domain"]);

    let combined = call(
        &client,
        json!({"text":"searchable", "node_types":["requirement", "rule"]}),
    )
    .await;
    let nodes = combined.structured_content.as_ref().unwrap()["nodes"]
        .as_array()
        .unwrap();
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
        assert!(
            readable.contains(node["id"].as_str().unwrap()),
            "{readable}"
        );
    }
    assert!(readable.contains("limit=2"), "{readable}");
    assert!(readable.contains(cursor), "{readable}");

    let second = call(
        &client,
        json!({"text":"shared", "limit":2, "cursor":cursor}),
    )
    .await;
    let first_ids = structured["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    let second_ids = second.structured_content.as_ref().unwrap()["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap())
        .collect::<Vec<_>>();
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
async fn list_denial_excludes_a_kind_before_selection_and_paging() {
    let repository = Repository::new("The shared graph is searchable.");
    repository.all_kinds();
    let restricted = access(&repository).deny_operation("list-sources");
    let host = StatementHost::with_fixture_access(restricted);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let source_only = call(&client, json!({"node_types":["source"]})).await;
    assert_eq!(source_only.is_error, Some(true), "{source_only:?}");
    assert_eq!(
        source_only.structured_content.as_ref().unwrap()["error"]["kind"],
        "access_denied"
    );

    let first = call(
        &client,
        json!({"node_types":["source", "requirement", "rule"], "limit":1}),
    )
    .await;
    let first_page = first.structured_content.as_ref().unwrap();
    assert_eq!(first_page["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(first_page["nodes"][0]["node_type"], "requirement");
    assert_eq!(first_page["has_more"], true);
    let cursor = first_page["next_cursor"].as_str().unwrap();
    assert!(!serde_json::to_string(&first)
        .unwrap()
        .contains("source_shared"));

    let second = call(
        &client,
        json!({
            "node_types":["source", "requirement", "rule"],
            "limit":1,
            "cursor":cursor
        }),
    )
    .await;
    let second_page = second.structured_content.as_ref().unwrap();
    assert_eq!(second_page["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(second_page["nodes"][0]["node_type"], "rule");
    assert_eq!(second_page["has_more"], false);
    assert!(second_page["next_cursor"].is_null());
    assert!(!serde_json::to_string(&second)
        .unwrap()
        .contains("source_shared"));

    let implicit = call(&client, json!({"text":"shared", "limit":20})).await;
    let implicit_page = implicit.structured_content.as_ref().unwrap();
    assert_eq!(implicit_page["nodes"].as_array().unwrap().len(), 7);
    assert!(implicit_page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|node| node["node_type"] != "source"));
    assert_eq!(implicit_page["has_more"], false);
    assert!(implicit_page["next_cursor"].is_null());
    assert!(!serde_json::to_string(&implicit)
        .unwrap()
        .contains("source_shared"));

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn list_grant_allows_search_when_get_is_denied() {
    let repository = Repository::new("The shared graph is searchable.");
    repository.all_kinds();
    let restricted = access(&repository).deny_operation("get-source");
    let host = StatementHost::with_fixture_access(restricted);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let result = call(&client, json!({"node_types":["source"]})).await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    assert_eq!(
        result.structured_content.as_ref().unwrap()["nodes"][0]["id"],
        "source_shared"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn no_list_search_grant_hides_and_refuses_named_search() {
    let repository = Repository::new("The shared graph is searchable.");
    let restricted = deny_operations(access(&repository), &LIST_SEARCH_OPERATIONS);
    let host = StatementHost::with_fixture_access(restricted);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    assert_search_hidden_and_refused(&client).await;

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn global_search_denial_hides_and_refuses_named_search() {
    let repository = Repository::new("The shared graph is searchable.");
    let restricted = access(&repository).deny_operation(catalog::Search::NAME);
    let host = StatementHost::with_fixture_access(restricted);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    assert_search_hidden_and_refused(&client).await;

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
