#![cfg(feature = "test-fixture")]

mod support { pub mod records; }

use provenance_transport::{fixture::{FixtureAccess, Target}, StatementHost};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;

fn access(repository: &Repository) -> FixtureAccess {
    FixtureAccess::new(
        vec![Target { id: "selected".into(), root: repository.dir.path().to_path_buf() }],
        vec![("selected".into(), "default".into())],
        "fixture-secret", "fixture.test",
    ).unwrap().allow_writes()
}

async fn call(client: &rmcp::service::RunningService<rmcp::RoleClient, ()>, name: &str, args: Value) -> rmcp::model::CallToolResult {
    client.call_tool(CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone())).await.unwrap()
}

#[tokio::test]
async fn named_mcp_discussion_actions_share_structured_and_readable_results() {
    let repository = Repository::new("A requirement has discussions.");
    let host = StatementHost::with_fixture_access(access(&repository));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let names = client.list_all_tools().await.unwrap().into_iter().map(|tool| tool.name.to_string()).collect::<Vec<_>>();
    for name in ["discussions", "discussion", "discuss", "reply"] { assert!(names.contains(&name.to_owned()), "{names:?}"); }

    let started = call(&client, "discuss", json!({
        "parent":{"node_type":"requirement","node_id":"req_shared"},
        "request_id":"request_start", "actor":"ben", "role":"user", "body":"Opening text"
    })).await;
    assert_ne!(started.is_error, Some(true), "{started:?}");
    let receipt = &started.structured_content.as_ref().unwrap()["receipt"];
    let id = receipt["discussion_id"].as_str().unwrap();
    let list = call(&client, "discussions", json!({})).await;
    let entry = &list.structured_content.as_ref().unwrap()["result"]["entries"][0];
    assert_eq!(entry["discussion_id"], id);
    assert_eq!(entry["opening_excerpt"], "Opening text");
    let readable = &list.content[0].as_text().unwrap().text;
    assert!(readable.contains(id) && readable.contains("limit=50"), "{readable}");

    let conversation = call(&client, "discussion", json!({"discussion_id":id})).await;
    assert_eq!(conversation.structured_content.as_ref().unwrap()["result"]["head"]["version"], 1);
    let replied = call(&client, "reply", json!({
        "discussion_id":id, "request_id":"request_reply", "actor":"ben",
        "expected_version":1, "role":"user", "body":"Second message"
    })).await;
    assert_ne!(replied.is_error, Some(true), "{replied:?}");
    assert_eq!(replied.structured_content.as_ref().unwrap()["receipt"]["version"], 2);
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn denied_parent_grant_is_applied_before_discussion_page_selection() {
    let repository = Repository::new("A requirement has discussions.");
    repository.all_kinds();
    let host = StatementHost::with_fixture_access(access(&repository).deny_operation("sources-list-discussions"));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let denied = call(&client, "discussions", json!({
        "parent":{"node_type":"source","node_id":"source_shared"}, "limit":1
    })).await;
    assert_eq!(denied.is_error, Some(true));
    let injection = call(&client, "discussions", json!({"allowed_parent_kinds":["source"]})).await;
    assert_eq!(injection.is_error, Some(true));
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
