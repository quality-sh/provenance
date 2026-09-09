#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
mod records;
use records::{call, get_call, host, Repository};
use rmcp::{model::CallToolRequestParams, ServiceExt};
use serde_json::{json, Value};

fn tool(operation: &str, call: &Value) -> CallToolRequestParams {
    CallToolRequestParams::new(operation.to_owned()).with_arguments(
        json!({"protocol_version":7,"call":call})
            .as_object()
            .unwrap()
            .clone(),
    )
}

#[tokio::test]
async fn real_mcp_preserves_each_registered_read_and_full_http_stamp() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&[("first", &repo)], &["first"]);
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
        [
            "check-statement",
            "plan",
            "info",
            "get",
            "search",
            "neighbors",
            "trace",
            "impact",
            "resolve-symbol",
            "evidence",
            "stale",
            "verification-runs",
            "verification-bindings",
            "list-threads",
            "list-messages",
            "list-proposals",
            "list-dispositions",
            "list-assertions"
        ]
    );
    let mut requests = vec![(
        "info",
        json!({"context":{"repository":"first"},"request":{}}),
    )];
    for (kind, id) in [
        ("domain", "domain_shared"),
        ("boundary", "boundary_shared"),
        ("requirement", "req_shared"),
        ("rule", "rule_shared"),
        ("source", "source_shared"),
        ("resolution", "resolution_shared"),
        ("topic", "topic_shared"),
        ("question", "question_shared"),
    ] {
        requests.push(("get", json!({"context":{"repository":"first","scope":"default"},"request":{"node_type":kind,"id":id}})));
    }
    for (operation, request) in [
        ("search", json!({"text":"shared","limit":1})),
        ("neighbors", json!({"id":"req_shared","limit":1})),
        ("trace", json!({"id":"rule_shared","limit":1})),
    ] {
        requests.push((
            operation,
            json!({"context":{"repository":"first","scope":"default"},"request":request}),
        ));
    }
    for (operation, request) in requests {
        let (status, expected) = call(&host, operation, request.clone()).await;
        assert_eq!(status, 200, "{expected}");
        let actual = client.call_tool(tool(operation, &request)).await.unwrap();
        assert_ne!(actual.is_error, Some(true));
        assert_eq!(actual.structured_content.unwrap(), expected, "{operation}");
    }
    let denied = client
        .call_tool(tool("get", &get_call("missing", "default")))
        .await
        .unwrap();
    assert_eq!(denied.is_error, Some(true));
    assert_eq!(
        denied.structured_content.unwrap()["error"]["kind"],
        "unknown_target"
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
    host.shutdown().await;
}

#[tokio::test]
async fn denied_mcp_tool_is_unadvertised_and_creates_no_storage() {
    use provenance_transport::{
        fixture::{FixtureAccess, Target},
        StatementHost,
    };
    let repo = Repository::new("The shared graph is readable.");
    let before = repo.bytes();
    let access = FixtureAccess::new(
        vec![Target {
            id: "first".into(),
            root: repo.dir.path().to_path_buf(),
        }],
        vec![("first".into(), "default".into())],
        "token",
        "fixture.test",
    )
    .unwrap()
    .deny_operation("get");
    let host = StatementHost::with_fixture_access(access);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    assert!(client
        .list_all_tools()
        .await
        .unwrap()
        .iter()
        .all(|tool| tool.name != "get"));
    let failure = client
        .call_tool(tool("get", &get_call("first", "default")))
        .await
        .unwrap();
    assert_eq!(failure.is_error, Some(true));
    assert_eq!(
        failure.structured_content.unwrap()["error"]["kind"],
        "access_denied"
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
    host.shutdown().await;
    assert_eq!(repo.bytes(), before);
}
