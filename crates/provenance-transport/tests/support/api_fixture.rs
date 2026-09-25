//! Fixture and MCP-call helpers shared by the api conformance tests.
use provenance_transport::fixture::{FixtureAccess, Target};
use provenance_transport::StatementHost;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use rmcp::service::RunningService;
use rmcp::ServiceExt as _;
use serde_json::Value;
use tokio::task::JoinHandle;

pub use super::records::Repository;

/// One fully granted fixture principal bound to the repository.
pub fn access(repository: &Repository) -> FixtureAccess {
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

/// One read-only fixture host over the repository.
pub fn host(repository: &Repository) -> StatementHost {
    StatementHost::with_fixture_access(access(repository))
}

/// The canonical failure kind of one refused tool result.
pub fn error_kind(result: &CallToolResult) -> String {
    result.structured_content.as_ref().unwrap()["error"]["kind"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// One connected MCP client session against one served host.
pub struct ApiSession {
    client: RunningService<rmcp::RoleClient, ()>,
    server: JoinHandle<RunningService<rmcp::RoleServer, StatementHost>>,
}

impl ApiSession {
    /// Serve one host over an in-memory stream pair and connect a client.
    pub async fn start(host: StatementHost) -> Self {
        let (client_io, server_io) = tokio::io::duplex(256 * 1024);
        let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
        let client = ().serve(client_io).await.unwrap();
        Self { client, server }
    }

    /// Call the api tool with structured arguments.
    pub async fn call(&self, arguments: Value) -> CallToolResult {
        self.client
            .call_tool(
                CallToolRequestParams::new("api")
                    .with_arguments(arguments.as_object().unwrap().clone()),
            )
            .await
            .unwrap()
    }

    /// Call one named catalog tool with structured arguments.
    pub async fn call_named(&self, name: &str, arguments: Value) -> CallToolResult {
        self.client
            .call_tool(
                CallToolRequestParams::new(name.to_owned()).with_arguments(
                    arguments.as_object().cloned().unwrap_or_default(),
                ),
            )
            .await
            .unwrap()
    }

    /// The listed tools of the served host.
    pub async fn tools(&self) -> Vec<Tool> {
        self.client.list_all_tools().await.unwrap()
    }

    /// End the client session and wait for the served host to stop.
    pub async fn shutdown(self) {
        self.client.cancel().await.unwrap();
        self.server.await.unwrap().cancel().await.unwrap();
    }
}
