use provenance_transport::StatementHost;
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::json;

#[tokio::test]
async fn mcp_projects_the_same_resource_operation() {
    let host = StatementHost::default();
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server =
        tokio::spawn(async move { provenance_mcp::serve(server_host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert!(!tools.iter().any(|tool| {
        provenance_contract::definitions()
            .iter()
            .any(|definition| definition.name == tool.name)
    }));
    let tool = tools.iter().find(|tool| tool.name == "api").unwrap();
    assert!(tool.input_schema["properties"]
        .get("protocol_version")
        .is_none());
    let result = client
        .call_tool(
            CallToolRequestParams::new("api").with_arguments(
                json!({
                    "path":"statement-checks",
                    "method":"post",
                    "body":{"statement":"The system stores records."}
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    assert!(result.structured_content.unwrap()["data"].is_object());
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
