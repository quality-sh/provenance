#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;

async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &str,
    arguments: Value,
) -> rmcp::model::CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(name)
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn mcp_target_first_authoring_uses_registered_schemas_and_structured_targets() {
    let repository = Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, true);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    for name in ["create", "update", "answer", "claim", "release", "submit"] {
        let tool = tools.iter().find(|tool| tool.name == name).unwrap();
        assert!(tool.output_schema.is_some(), "{name} output schema");
    }
    let create = tools.iter().find(|tool| tool.name == "create").unwrap();
    let create_schema = Value::Object(create.input_schema.clone());
    assert!(create_schema.to_string().contains("source_type"));
    assert!(create_schema.to_string().contains("refines"));
    assert!(create_schema.to_string().contains("\"target\""));
    assert!(create_schema.to_string().contains("\"type\""));

    let created = call(
        &client,
        "create",
        json!({
            "target":"source_mcp_target",
            "type":"source",
            "data":{
                "name":"MCP target",
                "source_type":"policy",
                "supersedes":[]
            }
        }),
    )
    .await;
    assert_ne!(created.is_error, Some(true), "{created:?}");
    assert_eq!(
        created.structured_content.as_ref().unwrap()["data"]["id"],
        "source_mcp_target"
    );
    assert!(created.content[0]
        .as_text()
        .unwrap()
        .text
        .contains("source_mcp_target"));

    let edited = call(
        &client,
        "update",
        json!({"target":"source_mcp_target","data":{"reference":"section 1"}}),
    )
    .await;
    assert_ne!(edited.is_error, Some(true), "{edited:?}");
    assert_eq!(
        edited.structured_content.as_ref().unwrap()["data"]["name"],
        "MCP target"
    );
    assert_eq!(
        edited.structured_content.as_ref().unwrap()["data"]["reference"],
        "section 1"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_named_actions_infer_kind_and_keep_domain_refusals() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = support::resource_http::host(&repository, true);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let claimed = call(
        &client,
        "claim",
        json!({"target":"topic_shared","data":{"actor":"worker"}}),
    )
    .await;
    assert_ne!(claimed.is_error, Some(true), "{claimed:?}");
    assert_eq!(
        claimed.structured_content.as_ref().unwrap()["data"]["claimed_by"],
        "worker"
    );

    let released = call(&client, "release", json!({"target":"topic_shared"})).await;
    assert_ne!(released.is_error, Some(true), "{released:?}");
    assert!(released.structured_content.as_ref().unwrap()["data"]["claimed_by"].is_null());

    let answered = call(
        &client,
        "answer",
        json!({"target":"question_shared","data":{"answer":"The shared answer."}}),
    )
    .await;
    assert_ne!(answered.is_error, Some(true), "{answered:?}");
    assert_eq!(
        answered.structured_content.as_ref().unwrap()["data"]["status"],
        "answered"
    );

    let invalid = call(
        &client,
        "claim",
        json!({"target":"question_shared","data":{"actor":"worker"}}),
    )
    .await;
    assert_eq!(invalid.is_error, Some(true), "{invalid:?}");

    let missing = call(
        &client,
        "update",
        json!({"target":"record_missing","data":{"name":"Missing"}}),
    )
    .await;
    assert_eq!(missing.is_error, Some(true), "{missing:?}");
    assert_eq!(
        missing.structured_content.unwrap()["error"]["kind"],
        "not_found"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
