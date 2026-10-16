#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    #[allow(dead_code)]
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
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

#[tokio::test]
#[provenance_macros::verifies("rule_ownership_refusal_names_owner", examples)]
async fn mcp_ownership_refusal_preserves_owner_and_guidance() {
    let repository = Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, true);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    for owner in [None, Some("spec://owner")] {
        let target = if owner.is_some() {
            "res_owned"
        } else {
            "res_manual"
        };
        let created = call(&client, "create", json!({"target":target,"type":"resolution","data":{
            "title":"Owner", "position":"Keep the owner.", "rationale":"The owner controls changes.",
            "made_by":owner, "supersedes":[], "requirement_ids":["req_shared"], "status":"active", "inputs":[]
        }})).await;
        assert_ne!(created.is_error, Some(true), "{created:?}");
        let result = call(&client, "submit", json!({"target":target,"data":{
            "declared_by":"other", "actor":"agent", "title":"Review", "summary":"Review the record.",
            "source_ids":[], "evidence_references":[], "builds_on":[]
        }})).await;
        assert_eq!(result.is_error, Some(true), "{result:?}");
        let error = &result.structured_content.as_ref().unwrap()["error"];
        assert_eq!(error["kind"], "record_ownership_conflict");
        assert_eq!(error["owner"], json!(owner));
        let message = error["message"].as_str().unwrap();
        assert!(message.contains("--declared-by"), "{message}");
        if let Some(owner) = owner {
            assert!(
                message.contains(owner) && message.contains("ask the owner"),
                "{message}"
            );
        } else {
            assert!(
                message.contains("omit --declared-by") && !message.contains("ask the owner"),
                "{message}"
            );
        }
    }
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_submit_names_all_missing_lists_before_store_checks() {
    let repository = Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, true);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let fields = ["source_ids", "evidence_references", "builds_on"];
    for supplied in 0..3 {
        let mut data = json!({"actor":"agent", "declared_by":"other", "title":"Review", "summary":"Review the record."});
        for field in &fields[..supplied] {
            data[*field] = json!([]);
        }
        let result = call(
            &client,
            "submit",
            json!({"target":"req_shared", "data":data}),
        )
        .await;
        assert_eq!(result.is_error, Some(true), "{result:?}");
        let error = &result.structured_content.as_ref().unwrap()["error"];
        assert_eq!(error["kind"], "invalid_input");
        for field in &fields[supplied..] {
            assert!(error.to_string().contains(field), "{error}");
            assert!(
                error
                    .to_string()
                    .contains(&format!("--{}-json '[]'", field.replace('_', "-"))),
                "{error}"
            );
        }
    }
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
