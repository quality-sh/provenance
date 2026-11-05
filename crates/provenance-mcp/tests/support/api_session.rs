//! MCP-call helpers shared by the api conformance tests.
use provenance_transport::StatementHost;
use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};
use rmcp::service::RunningService;
use rmcp::ServiceExt as _;
use serde_json::Value;
use tokio::task::JoinHandle;

/// The canonical failure kind of one refused tool result.
#[allow(dead_code)]
pub fn error_kind(result: &CallToolResult) -> String {
    result.structured_content.as_ref().unwrap()["error"]["kind"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// One connected MCP client session against one served host.
pub struct ApiSession {
    client: RunningService<rmcp::RoleClient, ()>,
    server: JoinHandle<provenance_mcp::Session>,
}

impl ApiSession {
    /// Serve one host over an in-memory stream pair and connect a client.
    pub async fn start(host: StatementHost) -> Self {
        let (client_io, server_io) = tokio::io::duplex(256 * 1024);
        let server =
            tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
        let client = ().serve(client_io).await.unwrap();
        Self { client, server }
    }

    /// Call the api tool with structured arguments. A call can read or write.
    pub async fn call(&self, arguments: Value) -> CallToolResult {
        self.client
            .call_tool(
                CallToolRequestParams::new("api")
                    .with_arguments(arguments.as_object().unwrap().clone()),
            )
            .await
            .unwrap()
    }

    /// The listed tools of the served host.
    #[allow(dead_code)]
    pub async fn tools(&self) -> Vec<Tool> {
        self.client.list_all_tools().await.unwrap()
    }

    /// End the client session and wait for the served host to stop.
    pub async fn shutdown(self) {
        self.client.cancel().await.unwrap();
        self.server.await.unwrap().cancel().await.unwrap();
    }
}
