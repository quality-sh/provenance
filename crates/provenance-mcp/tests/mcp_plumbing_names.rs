//! MCP lists only porcelain tools. A plumbing operation name is an unknown
//! tool. The api tool reaches the operations that read, and the api action of
//! change reaches the operations that write.
#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/check_port.rs"]
    pub mod check_port;
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
}

use provenance_macros::verifies;
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::json;
use std::sync::Arc;
use support::{check_port::FixtureCheckPort, records::Repository};

fn full_host(repository: &Repository) -> StatementHost {
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
    StatementHost::with_fixture_access(access).with_check_port(Arc::new(FixtureCheckPort))
}

#[tokio::test]
#[verifies("rule_porcelain_plumbing_only_through_api", exhaustion)]
async fn no_plumbing_operation_name_is_a_listed_or_callable_tool() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = full_host(&repository);
    let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 7, "{tools:?}");
    for definition in provenance_contract::definitions() {
        assert!(
            !tools.iter().any(|tool| tool.name == definition.name),
            "{} is listed",
            definition.name
        );
        let refused = client
            .call_tool(
                CallToolRequestParams::new(definition.name)
                    .with_arguments(json!({}).as_object().unwrap().clone()),
            )
            .await
            .expect_err(definition.name);
        let rmcp::ServiceError::McpError(error) = refused else {
            panic!("{}: {refused:?}", definition.name);
        };
        assert_eq!(error.code.0, -32602, "{}", definition.name);
        assert_eq!(
            error.message,
            format!("Unknown tool: {}", definition.name),
            "{}",
            definition.name
        );
    }

    let through_api = client
        .call_tool(
            CallToolRequestParams::new("api").with_arguments(
                json!({"path":"sources/source_shared"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_ne!(through_api.is_error, Some(true), "{through_api:?}");
    assert_eq!(
        through_api.structured_content.unwrap()["data"]["id"],
        "source_shared"
    );

    // A route that writes goes through the api action of change, not the api tool.
    let update = json!({"path":"sources/source_shared","method":"patch","body":{"name":"Renamed"}});
    let call = |name: &str, arguments: serde_json::Value| {
        client.call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
    };
    let refused = call("api", update.clone()).await.unwrap();
    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    assert_eq!(
        refused.structured_content.unwrap()["error"]["field"],
        "path"
    );
    let written = call("change", json!({"action":"api","request":update}))
        .await
        .unwrap();
    assert_ne!(written.is_error, Some(true), "{written:?}");
    assert_eq!(
        written.structured_content.unwrap()["data"]["name"],
        "Renamed"
    );
    let read_through_change = call(
        "change",
        json!({"action":"api","request":{"path":"sources/source_shared"}}),
    )
    .await
    .unwrap();
    assert_eq!(
        read_through_change.structured_content.unwrap()["error"]["field"],
        "request.path"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
