use provenance_transport::StatementHost;
use rmcp::{model::CallToolRequestParams, ServiceExt};
use serde_json::{json, Value};

fn statement_call(arguments: Value) -> CallToolRequestParams {
    let Value::Object(arguments) = arguments else {
        panic!("tool arguments must be an object")
    };
    CallToolRequestParams::new("check-statement").with_arguments(arguments)
}

#[tokio::test]
async fn actual_mcp_session_lists_contract_and_preserves_report_and_failure_channels() {
    let host = StatementHost::default();
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>(),
        ["check-statement"]
    );
    assert!(tools[0].output_schema.is_some());
    for statement in ["Install the cover.", "Stop; wait.", "Café; wait."] {
        let result = client
            .call_tool(statement_call(json!({
                "protocol_version":7,"call":{"request":{"statement":statement}}
            })))
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.unwrap(),
            serde_json::to_value(provenance_ste100::check_descriptive(statement)).unwrap()
        );
    }
    let refused = client
        .call_tool(statement_call(json!({
            "protocol_version":7,"call":{"request":{"statement":null}}
        })))
        .await
        .unwrap();
    assert_eq!(refused.is_error, Some(true));
    assert_eq!(
        refused.structured_content.unwrap()["error"]["kind"],
        "invalid_input"
    );
    for arguments in [
        json!({"call":{"request":{"statement":"x"}}}),
        json!({"protocol_version":null,"call":{}}),
        json!({"protocol_version":7,"call":{},"extra":1}),
    ] {
        let malformed = client.call_tool(statement_call(arguments)).await;
        assert!(
            malformed.is_err(),
            "Malformed MCP frames must use the protocol error channel"
        );
    }
    let mismatch = client
        .call_tool(statement_call(json!({
            "protocol_version":6,"call":{"request":{"statement":"x"}}
        })))
        .await
        .unwrap();
    assert_eq!(mismatch.is_error, Some(true));
    assert_eq!(
        mismatch.structured_content.unwrap()["error"]["kind"],
        "protocol_mismatch"
    );
    let oversized = client
        .call_tool(statement_call(json!({
            "protocol_version":7,"call":{"request":{"statement":"a".repeat(2*1024*1024)}}
        })))
        .await
        .unwrap();
    assert_eq!(oversized.is_error, Some(true));
    assert_eq!(
        oversized.structured_content.unwrap()["error"]["reason"],
        "too_large"
    );
    let unknown = client
        .call_tool(CallToolRequestParams::new("unknown"))
        .await;
    assert!(
        unknown.is_err(),
        "Unknown tools must use the MCP protocol error channel"
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
    host.shutdown().await;
}

#[tokio::test]
async fn overlong_mcp_frame_refuses_before_json_parse_or_frame_end() {
    use tokio::io::AsyncWriteExt;
    let host = StatementHost::default();
    let (mut client_io, server_io) = tokio::io::duplex(64 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await });
    // No newline and no valid JSON: the reader must enforce its own frame limit.
    let _ = client_io.write_all(&vec![b'x'; 4 * 1024 * 1024 + 1]).await;
    let refusal = tokio::time::timeout(std::time::Duration::from_secs(2), server).await;
    assert!(
        refusal.is_ok(),
        "An overlong incomplete frame must not wait for more bytes"
    );
    assert!(refusal.unwrap().unwrap().is_err());
    host.shutdown().await;
}
