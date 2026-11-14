use provenance_transport::StatementHost;
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};

pub type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

pub struct Session {
    pub client: Client,
    server: tokio::task::JoinHandle<provenance_mcp::Session>,
}

impl Session {
    pub async fn start(host: StatementHost) -> Self {
        let (client_io, server_io) = tokio::io::duplex(256 * 1024);
        let server =
            tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
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

/// Accepts the pending submission of a record as the reviewer through the
/// `api` tool: the creation submission, or the status request that sets an
/// active or archived status. The repository must allow the reviewer.
pub async fn accept_status_request(client: &Client, collection: &str, id: &str) {
    let read = call(
        client,
        "api",
        json!({"path": format!("{collection}/{id}"), "method": "get"}),
    )
    .await;
    assert_ne!(read.is_error, Some(true), "{read:?}");
    let record = &read.structured_content.as_ref().unwrap()["data"];
    let proposal = record["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no pending submission: {record}"));
    let kind = collection.trim_end_matches('s');
    let decided = call(
        client,
        "api",
        json!({
            "path": format!("{collection}/{id}/submissions/{proposal}/decide"),
            "method": "post",
            "input": {
                "actor": {"identity_type": "human", "id": "reviewer"},
                "decision": "accepted", "rationale": null,
                "canonical_artifact": {"artifact_type": kind, "artifact_id": id},
                "feedback": null, "declared_by": null
            }
        }),
    )
    .await;
    assert_ne!(decided.is_error, Some(true), "{decided:?}");
}
