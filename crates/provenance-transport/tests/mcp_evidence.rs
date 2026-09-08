#![cfg(feature = "test-fixture")]
#[allow(dead_code)]
#[path = "support/records.rs"]
mod records;
use records::{call, host, Repository};
use rmcp::{model::CallToolRequestParams, ServiceExt};
use serde_json::json;

#[tokio::test]
async fn real_mcp_keeps_all_evidence_fields_stamps_and_complete_list_wrapper() {
    let repo = Repository::new("The evidence is readable.");
    let base = repo.evidence();
    let host = host(&[("selected", &repo)], &["selected"]);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let definitions = client.list_all_tools().await.unwrap();
    for (operation, request) in [
        ("impact", json!({"id":"rule_shared"})),
        ("resolve-symbol", json!({"file":"code.rs"})),
        (
            "evidence",
            json!({"rule":"rule_shared","limit":1,"base":base}),
        ),
        ("stale", json!({"base":base,"rules":["rule_shared"]})),
        ("verification-runs", json!({})),
        ("verification-bindings", json!({})),
    ] {
        let body = json!({"context":{"repository":"selected","scope":"default"},"request":request});
        let (status, expected) = call(&host, operation, body.clone()).await;
        assert_eq!(status, 200, "{expected}");
        let answer = client
            .call_tool(
                CallToolRequestParams::new(operation).with_arguments(
                    json!({"protocol_version":7,"call":body})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            )
            .await
            .unwrap();
        assert_ne!(answer.is_error, Some(true));
        let expected = if expected.is_array() {
            json!({"result":expected})
        } else {
            expected
        };
        assert_eq!(answer.structured_content.unwrap(), expected, "{operation}");
        let schema = definitions
            .iter()
            .find(|entry| entry.name == operation)
            .unwrap()
            .output_schema
            .as_ref()
            .unwrap();
        assert_eq!(schema["type"], "object");
        if operation.starts_with("verification-") {
            assert_eq!(schema["properties"]["result"]["type"], "array");
        }
    }
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
    host.shutdown().await;
}
