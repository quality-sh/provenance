//! MCP lists only porcelain tools. A plumbing operation name is an unknown
//! tool, and the api tool still reaches the operation.
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
    assert_eq!(tools.len(), 14, "{tools:?}");
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
        assert_eq!(error.code.0, -32601, "{}", definition.name);
        assert_eq!(error.message, "Unknown tool", "{}", definition.name);
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

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
