use provenance_transport::StatementHost;
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::Value;

pub type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

pub struct Session {
    pub client: Client,
    server: tokio::task::JoinHandle<rmcp::service::RunningService<rmcp::RoleServer, StatementHost>>,
}

impl Session {
    pub async fn start(host: StatementHost) -> Self {
        let (client_io, server_io) = tokio::io::duplex(256 * 1024);
        let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
        let client = ().serve(client_io).await.unwrap();
        Self { client, server }
    }

    pub async fn close(self) {
        self.client.cancel().await.unwrap();
        self.server.await.unwrap().cancel().await.unwrap();
    }
}

pub async fn call(client: &Client, name: &str, arguments: Value) -> rmcp::model::CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}
