#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
mod records;
use provenance_core::{MessageRole, NodeType, ScopeId, StableId, ThreadParent};
use provenance_store::state_store::{PostMessageInput, StateStore};
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
    let scope = ScopeId::new("default").unwrap();
    let store = StateStore::new(repo.layout.clone());
    store
        .post_thread_message(PostMessageInput {
            scope_id: scope.clone(),
            parent: ThreadParent {
                node_type: NodeType::Requirement,
                node_id: StableId::new("req_shared").unwrap(),
            },
            role: MessageRole::User,
            body: "Read the complete document.".into(),
        })
        .unwrap();
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
            "read-document",
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
        ("read-document", json!({"id":"req_shared"})),
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
        if operation == "read-document" {
            assert_document_matches_store(&expected, &store, &scope);
        }
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

fn assert_document_matches_store(answer: &Value, store: &StateStore, scope: &ScopeId) {
    assert_eq!(answer["operation"], "read-document");
    assert_eq!(
        answer["protocol_version"],
        provenance_core::SDK_PROTOCOL_VERSION
    );
    assert_eq!(answer["root_id"], "req_shared");
    for (family, rows) in [
        (
            "requirements",
            json!(store.list_requirements(scope).unwrap()),
        ),
        ("resolutions", json!(store.list_resolutions(scope).unwrap())),
        ("rules", json!(store.list_rules(scope).unwrap())),
        ("sources", json!(store.list_sources(scope).unwrap())),
        ("topics", json!(store.list_topics(scope).unwrap())),
        ("questions", json!(store.list_questions(scope).unwrap())),
        ("threads", json!(store.list_threads(scope).unwrap())),
        ("messages", json!(store.list_messages(scope).unwrap())),
    ] {
        assert!(!rows.as_array().unwrap().is_empty(), "{family}");
        assert_eq!(answer[family], rows, "{family}");
    }
    assert_eq!(answer["stamp"]["policy"], "catch_up");
    assert_eq!(
        answer["stamp"]["attested"],
        json!([
            "messages",
            "questions",
            "requirements",
            "resolutions",
            "rules",
            "sources",
            "threads",
            "topics"
        ])
    );
    assert_eq!(answer["stamp"]["live"], json!([]));
}

#[tokio::test]
async fn document_transports_preserve_scope_freshness_and_refusals() {
    let repo = Repository::new("The default graph is readable.");
    repo.add_scope("other", "The other graph is readable.");
    let host = host(&[("selected", &repo)], &["selected"]);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let request =
        json!({"context":{"repository":"selected","scope":"other"},"request":{"id":"req_shared"}});
    let (status, first) = call(&host, "read-document", request.clone()).await;
    assert_eq!(status, 200, "{first}");
    assert_eq!(
        first["rules"][0]["statement"],
        "The other graph is readable."
    );
    assert_eq!(first["rules"][0]["scope_id"], "other");
    for (body, status, kind) in [
        (
            json!({"context":{"repository":"missing","scope":"other"},"request":{"id":"req_shared"}}),
            404,
            "unknown_target",
        ),
        (
            json!({"context":{"repository":"selected","scope":"denied"},"request":{"id":"req_shared"}}),
            403,
            "access_denied",
        ),
        (
            json!({"context":{"repository":"selected","scope":"other"},"request":{"id":""}}),
            400,
            "invalid_input",
        ),
        (
            json!({"context":{"repository":"selected","scope":"other"},"request":{"id":"req_shared","limit":1}}),
            400,
            "invalid_input",
        ),
    ] {
        let (actual_status, expected) = call(&host, "read-document", body.clone()).await;
        assert_eq!(actual_status, status, "{expected}");
        assert_eq!(expected["error"]["kind"], kind);
        let actual = client
            .call_tool(tool("read-document", &body))
            .await
            .unwrap();
        assert_eq!(actual.is_error, Some(true));
        assert_eq!(actual.structured_content.unwrap(), expected);
    }
    repo.edit("other", "The saved edit is readable.");
    let mut stale_request = request.clone();
    stale_request["context"]["freshness"] = json!("refuse_stale");
    let (status, stale) = call(&host, "read-document", stale_request.clone()).await;
    assert_eq!(status, 409, "{stale}");
    assert_eq!(stale["error"]["kind"], "stale");
    assert_eq!(stale["error"]["digest"], first["stamp"]["digest"]);
    let actual = client
        .call_tool(tool("read-document", &stale_request))
        .await
        .unwrap();
    assert_eq!(actual.is_error, Some(true));
    assert_eq!(actual.structured_content.unwrap(), stale);
    let (status, refreshed) = call(&host, "read-document", request.clone()).await;
    assert_eq!(status, 200, "{refreshed}");
    assert_eq!(
        refreshed["rules"][0]["statement"],
        "The saved edit is readable."
    );
    assert_ne!(refreshed["stamp"]["digest"], first["stamp"]["digest"]);
    let actual = client
        .call_tool(tool("read-document", &request))
        .await
        .unwrap();
    assert_ne!(actual.is_error, Some(true));
    assert_eq!(actual.structured_content.unwrap(), refreshed);
    let scope = provenance_core::ScopeId::new("other").unwrap();
    std::fs::write(
        provenance_store::shards::rules_path(&repo.layout, &scope),
        "invalid JSON\n",
    )
    .unwrap();
    let (status, failed) = call(&host, "read-document", request.clone()).await;
    assert_eq!(status, 200, "{failed}");
    assert_eq!(failed["stamp"]["policy"], "catch_up_failed");
    assert_eq!(failed["stamp"]["digest"], refreshed["stamp"]["digest"]);
    assert_eq!(failed["rules"], refreshed["rules"]);
    assert_eq!(failed["freshness_cause"], "catch_up_failed");
    assert_eq!(
        failed["freshness_error"],
        "catch-up failed; answer uses the stored projection"
    );
    let actual = client
        .call_tool(tool("read-document", &request))
        .await
        .unwrap();
    assert_ne!(actual.is_error, Some(true));
    assert_eq!(actual.structured_content.unwrap(), failed);
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
