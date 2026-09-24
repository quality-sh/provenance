#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use provenance_macros::verifies;
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;

fn access(repository: &Repository) -> FixtureAccess {
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
    .allow_writes()
}

async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &str,
    args: Value,
) -> rmcp::model::CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

fn assert_tools(tools: &[rmcp::model::Tool]) {
    let names = tools
        .iter()
        .map(|tool| tool.name.to_string())
        .collect::<Vec<_>>();
    for name in ["discussions", "discussion", "discuss", "reply"] {
        assert!(names.contains(&name.to_owned()), "{names:?}");
    }
    for tool in tools
        .iter()
        .filter(|tool| names.iter().any(|name| name == tool.name.as_ref()))
    {
        if ["discussions", "discussion", "discuss", "reply"].contains(&tool.name.as_ref()) {
            jsonschema::JSONSchema::compile(&json!(tool.input_schema)).unwrap();
            jsonschema::JSONSchema::compile(&json!(tool.output_schema.as_ref().unwrap())).unwrap();
        }
    }
}

fn assert_operation_page_metadata(
    list: &rmcp::model::CallToolResult,
    conversation: &rmcp::model::CallToolResult,
) {
    assert_eq!(
        list.structured_content.as_ref().unwrap()["result"]["limit"],
        50
    );
    assert_eq!(
        list.structured_content.as_ref().unwrap()["result"]["has_more"],
        false
    );
    assert_eq!(
        conversation.structured_content.as_ref().unwrap()["result"]["messages"]["limit"],
        50
    );
}

async fn assert_conversation_continuations(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    id: &str,
) {
    let other = call(
        client,
        "discuss",
        json!({
            "parent":{"node_type":"requirement","node_id":"req_shared"},
            "request_id":"request_other", "actor":"ben", "role":"user", "body":"Other root"
        }),
    )
    .await;
    let other_id = other.structured_content.as_ref().unwrap()["receipt"]["discussion_id"]
        .as_str()
        .unwrap();
    let first_page = call(client, "discussion", json!({"discussion_id":id,"limit":1})).await;
    let cursor = first_page.structured_content.as_ref().unwrap()["result"]["messages"]
        ["next_cursor"]
        .as_str()
        .unwrap();
    let wrong_selector = call(
        client,
        "discussion",
        json!({"discussion_id":other_id,"limit":1,"cursor":cursor}),
    )
    .await;
    assert_eq!(wrong_selector.is_error, Some(true));
    let next_page = call(
        client,
        "discussion",
        json!({"discussion_id":id,"limit":1,"cursor":cursor}),
    )
    .await;
    assert_eq!(
        next_page.structured_content.as_ref().unwrap()["result"]["messages"]["entries"][0]["body"],
        "Second message"
    );
    let stale = call(
        client,
        "reply",
        json!({
            "discussion_id":id, "request_id":"request_stale", "actor":"ben",
            "expected_version":1, "role":"user", "body":"Stale message"
        }),
    )
    .await;
    assert_eq!(stale.is_error, Some(true));
    let later = call(
        client,
        "reply",
        json!({
            "discussion_id":id, "request_id":"request_later", "actor":"ben",
            "expected_version":2, "role":"user", "body":"Later message"
        }),
    )
    .await;
    assert_ne!(later.is_error, Some(true), "{later:?}");
    let stale_cursor = call(
        client,
        "discussion",
        json!({"discussion_id":id,"limit":1,"cursor":cursor}),
    )
    .await;
    assert_eq!(stale_cursor.is_error, Some(true));
}

#[tokio::test]
#[verifies("rule_porcelain_discussion_targets", examples)]
async fn named_mcp_discussion_actions_share_structured_and_readable_results() {
    let repository = Repository::new("A requirement has discussions.");
    let host = StatementHost::with_fixture_access(access(&repository));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert_tools(&tools);

    let started = call(
        &client,
        "discuss",
        json!({
            "parent":{"node_type":"requirement","node_id":"req_shared"},
            "request_id":"request_start", "actor":"ben", "role":"user", "body":"Opening text"
        }),
    )
    .await;
    assert_ne!(started.is_error, Some(true), "{started:?}");
    let receipt = &started.structured_content.as_ref().unwrap()["receipt"];
    let write_tool = tools.iter().find(|tool| tool.name == "discuss").unwrap();
    assert!(
        jsonschema::JSONSchema::compile(&json!(write_tool.output_schema.as_ref().unwrap()))
            .unwrap()
            .is_valid(started.structured_content.as_ref().unwrap())
    );
    let id = receipt["discussion_id"].as_str().unwrap();
    let replay = call(
        &client,
        "discuss",
        json!({
            "parent":{"node_type":"requirement","node_id":"req_shared"},
            "request_id":"request_start", "actor":"ben", "role":"user", "body":"Opening text"
        }),
    )
    .await;
    assert_eq!(
        replay.structured_content.as_ref().unwrap()["receipt"],
        *receipt
    );
    let changed_intent = call(
        &client,
        "discuss",
        json!({
            "parent":{"node_type":"requirement","node_id":"req_shared"},
            "request_id":"request_start", "actor":"ben", "role":"user", "body":"Changed"
        }),
    )
    .await;
    assert_eq!(changed_intent.is_error, Some(true));
    let list = call(&client, "discussions", json!({})).await;
    assert_eq!(
        list.structured_content.as_ref().unwrap()["scope_id"],
        "default"
    );
    assert_eq!(
        list.structured_content.as_ref().unwrap()["status"],
        "active"
    );
    let list_tool = tools
        .iter()
        .find(|tool| tool.name == "discussions")
        .unwrap();
    assert!(
        jsonschema::JSONSchema::compile(&json!(list_tool.output_schema.as_ref().unwrap()))
            .unwrap()
            .is_valid(list.structured_content.as_ref().unwrap())
    );
    let entry = &list.structured_content.as_ref().unwrap()["result"]["entries"][0];
    assert_eq!(entry["discussion_id"], id);
    assert_eq!(entry["opening_excerpt"], "Opening text");
    let readable = &list.content[0].as_text().unwrap().text;
    assert!(
        readable.contains(id)
            && readable.contains("scope=default")
            && readable.contains("limit=50"),
        "{readable}"
    );

    let conversation = call(&client, "discussion", json!({"discussion_id":id})).await;
    assert_eq!(
        conversation.structured_content.as_ref().unwrap()["result"]["head"]["version"],
        1
    );
    assert_operation_page_metadata(&list, &conversation);
    let replied = call(
        &client,
        "reply",
        json!({
            "discussion_id":id, "request_id":"request_reply", "actor":"ben",
            "expected_version":1, "role":"user", "body":"Second message"
        }),
    )
    .await;
    assert_ne!(replied.is_error, Some(true), "{replied:?}");
    assert_eq!(
        replied.structured_content.as_ref().unwrap()["receipt"]["version"],
        2
    );
    assert_conversation_continuations(&client, id).await;
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn reply_grant_does_not_admit_start() {
    let repository = Repository::new("A requirement has discussions.");
    let store = provenance_store::state_store::StateStore::new(repository.layout.clone());
    let request = serde_json::from_value(json!({
        "scope_id":"default", "request_id":"initial_start", "actor":"ben",
        "declared_by":null,
        "parent":{"node_type":"requirement","node_id":"req_shared"},
        "action":{"kind":"start","role":"user","body":"Opening text"}
    }))
    .unwrap();
    let receipt = store.write_discussion(request).unwrap();
    let host = StatementHost::with_fixture_access(
        access(&repository).deny_operation("requirements-create-discussion"),
    );
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let names = client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect::<Vec<_>>();
    assert!(names.contains(&"discuss".to_owned()));
    assert!(names.contains(&"reply".to_owned()));
    let denied = call(
        &client,
        "discuss",
        json!({
            "parent":{"node_type":"requirement","node_id":"req_shared"},
            "request_id":"denied_start", "actor":"ben", "role":"user", "body":"Denied start"
        }),
    )
    .await;
    assert_eq!(denied.is_error, Some(true));
    let reply = call(
        &client,
        "reply",
        json!({
            "discussion_id":receipt.discussion_id, "request_id":"reply_only", "actor":"ben",
            "expected_version":1, "role":"user", "body":"Permitted reply"
        }),
    )
    .await;
    assert_ne!(reply.is_error, Some(true), "{reply:?}");
    assert_eq!(
        reply.structured_content.as_ref().unwrap()["receipt"]["version"],
        2
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn denied_parent_grant_is_applied_before_discussion_page_selection() {
    let repository = Repository::new("A requirement has discussions.");
    repository.all_kinds();
    let store = provenance_store::state_store::StateStore::new(repository.layout.clone());
    let mut source = None;
    for (request_id, node_type, node_id) in [
        ("req_start", "requirement", "req_shared"),
        ("source_start", "source", "source_shared"),
    ] {
        let request = serde_json::from_value(json!({
            "scope_id":"default", "request_id":request_id, "actor":"ben",
            "declared_by":null,
            "parent":{"node_type":node_type,"node_id":node_id},
            "action":{"kind":"start","role":"user","body":request_id}
        }))
        .unwrap();
        let receipt = store.write_discussion(request).unwrap();
        if node_type == "source" {
            source = Some(receipt);
        }
    }
    let source = source.unwrap();
    let host = StatementHost::with_fixture_access(
        access(&repository)
            .deny_operation("sources-list-discussions")
            .deny_operation("sources-get-discussion")
            .deny_operation("sources-create-discussion")
            .deny_operation("sources-create-discussion-message"),
    );
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let denied = call(
        &client,
        "discussions",
        json!({
            "parent":{"node_type":"source","node_id":"source_shared"}, "limit":1
        }),
    )
    .await;
    assert_eq!(denied.is_error, Some(true));
    let page = call(&client, "discussions", json!({"limit":1})).await;
    let result = &page.structured_content.as_ref().unwrap()["result"];
    assert_eq!(result["entries"][0]["parent"]["node_type"], "requirement");
    assert_eq!(page.structured_content.as_ref().unwrap()["has_more"], false);
    let hidden = call(
        &client,
        "discussion",
        json!({"discussion_id":source.discussion_id}),
    )
    .await;
    assert_eq!(hidden.is_error, Some(true));
    let denied_reply = call(
        &client,
        "reply",
        json!({
            "discussion_id":source.discussion_id, "request_id":"denied_reply", "actor":"ben",
            "expected_version":1, "role":"user", "body":"Denied"
        }),
    )
    .await;
    assert_eq!(denied_reply.is_error, Some(true));
    let injection = call(
        &client,
        "discussions",
        json!({"allowed_parent_kinds":["source"]}),
    )
    .await;
    assert_eq!(injection.is_error, Some(true));
    for limit in [0, 201] {
        let invalid = call(&client, "discussions", json!({"limit":limit})).await;
        assert_eq!(invalid.is_error, Some(true));
    }
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
