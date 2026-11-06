#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/records.rs"]
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

fn writable_access(repository: &Repository) -> FixtureAccess {
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
    .allow_writes()
}

async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &str,
    arguments: Value,
) -> rmcp::model::CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

/// Whether the change tool lists `action`.
fn listed(tools: &[rmcp::model::Tool], action: &str) -> bool {
    tools
        .iter()
        .find(|tool| tool.name == "change")
        .and_then(|change| {
            change.input_schema["properties"]["action"]["enum"]
                .as_array()
                .cloned()
        })
        .is_some_and(|actions| actions.contains(&json!(action)))
}

/// The record kinds that the create action of the change tool takes.
fn create_kinds(tools: &[rmcp::model::Tool]) -> Value {
    let change = tools.iter().find(|tool| tool.name == "change").unwrap();
    change.input_schema["properties"]["type"]["enum"].clone()
}

#[tokio::test]
async fn non_create_schema_excludes_a_writable_but_unreadable_kind() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let access = writable_access(&repository).deny_operation("get-source");
    let host = StatementHost::with_fixture_access(access);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    assert!(listed(&tools, "update"));

    let unresolved = call(
        &client,
        "change",
        json!({"action":"update","target":"source_shared","data":{"name":"Hidden source"}}),
    )
    .await;
    assert_eq!(unresolved.is_error, Some(true), "{unresolved:?}");
    assert_eq!(
        unresolved.structured_content.unwrap()["error"]["kind"],
        "not_found"
    );

    let direct = call(
        &client,
        "api",
        json!({
            "path":"sources/source_shared",
            "method":"patch",
            "input":{"name":"Direct source update"}
        }),
    )
    .await;
    assert_ne!(direct.is_error, Some(true), "{direct:?}");
    assert_eq!(
        direct.structured_content.unwrap()["data"]["name"],
        "Direct source update"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn non_create_schema_and_dispatch_deny_a_hidden_mutation() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let access = writable_access(&repository).deny_operation("update-source");
    let host = StatementHost::with_fixture_access(access);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    assert!(listed(&tools, "update"));

    let denied = call(
        &client,
        "change",
        json!({"action":"update","target":"source_shared","data":{"name":"Denied update"}}),
    )
    .await;
    assert_eq!(denied.is_error, Some(true), "{denied:?}");
    assert_eq!(
        denied.structured_content.unwrap()["error"]["kind"],
        "access_denied"
    );
    let source = call(&client, "api", json!({"path":"sources/source_shared"})).await;
    assert_ne!(source.is_error, Some(true), "{source:?}");
    assert_eq!(
        source.structured_content.unwrap()["data"]["name"],
        "Shared source"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn no_resolvable_kind_hides_non_create_tools_but_keeps_create_executable() {
    let repository = Repository::new("The shared graph is readable.");
    let mut access = writable_access(&repository);
    for definition in provenance_contract::definitions()
        .iter()
        .filter(|definition| {
            definition.registration.queries.iter().any(|query| {
                query.name == catalog::Trace::NAME && query.request.node_type.is_some()
            })
        })
    {
        access = access.deny_operation(definition.name);
    }
    let host = StatementHost::with_fixture_access(access);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    for name in ["update", "answer", "claim", "release", "submit"] {
        assert!(!listed(&tools, name), "{name}");
    }
    assert!(listed(&tools, "create"));
    assert!(create_kinds(&tools)
        .as_array()
        .unwrap()
        .contains(&json!("source")));

    let created = call(
        &client,
        "change",
        json!({"action":"create",
            "target":"source_without_read_access",
            "type":"source",
            "data":{
                "name":"Write-only source",
                "source_type":"policy",
                "supersedes":[]
            }
        }),
    )
    .await;
    assert_ne!(created.is_error, Some(true), "{created:?}");
    assert_eq!(
        created.structured_content.unwrap()["data"]["id"],
        "source_without_read_access"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn hidden_resolver_hides_non_create_tools_without_hiding_create() {
    let repository = Repository::new("The shared graph is readable.");
    let access = writable_access(&repository).deny_operation(catalog::ResolveRecord::NAME);
    let host = StatementHost::with_fixture_access(access);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    assert!(listed(&tools, "create"));
    for name in ["update", "answer", "claim", "release", "submit"] {
        assert!(!listed(&tools, name), "{name}");
    }

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
