#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use provenance_porcelain::check::{Category, CheckPort, PortFuture};
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use std::sync::Arc;
use support::records::Repository;

struct UnusedCheckPort;

impl CheckPort for UnusedCheckPort {
    fn run<'a>(&'a self, _: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async { Err("unused test port".to_owned()) })
    }
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

fn assert_error_envelope(name: &str, result: &rmcp::model::CallToolResult) {
    assert_eq!(result.is_error, Some(true), "{name}: {result:?}");
    let envelope = result
        .structured_content
        .as_ref()
        .and_then(Value::as_object)
        .unwrap();
    let mut fields = envelope.keys().map(String::as_str).collect::<Vec<_>>();
    fields.sort_unstable();
    assert_eq!(fields, ["error", "meta"], "{name}: {envelope:?}");
    assert!(envelope["error"].is_object(), "{name}: {envelope:?}");
    assert!(envelope["meta"].is_object(), "{name}: {envelope:?}");
}

#[tokio::test]
async fn every_mcp_tool_family_uses_the_error_envelope() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repository.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
    .allow_writes();
    let host = StatementHost::with_fixture_access(access)
        .with_check_port(Arc::new(UnusedCheckPort));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    for (family, tool, arguments) in [
        ("catalog", "get-source", json!({})),
        ("get", "get", json!({"unknown":true})),
        ("search", "search", json!({"unknown":true})),
        ("api", "api", json!({"method":"post"})),
        ("authoring", "create", json!({})),
        ("discussion", "discussion", json!({})),
        ("check", "check", json!({"unknown":true})),
    ] {
        assert_error_envelope(family, &call(&client, tool, arguments).await);
    }

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
