#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
#[allow(dead_code)]
mod records;
use provenance_core::{MessageRole, NodeType, ScopeId, StableId, ThreadParent};
use provenance_store::state_store::{PostMessageInput, StateStore};
use records::{call, Repository};
use serde_json::{json, Value};
use std::fmt::Write;

fn writable_host(repo: &Repository) -> provenance_transport::StatementHost {
    use provenance_transport::fixture::{FixtureAccess, Target};
    provenance_transport::StatementHost::with_fixture_access(
        FixtureAccess::new(
            vec![Target {
                id: "selected".into(),
                root: repo.dir.path().into(),
            }],
            vec![("selected".into(), "default".into())],
            "fixture-secret",
            "fixture.test",
        )
        .unwrap()
        .allow_writes(),
    )
}
fn scoped(request: &Value) -> Value {
    json!({"context":{"repository":"selected","scope":"default"},"request":request})
}
fn input(body: &str) -> Value {
    json!({"scope_id":"default","parent":{"node_type":"requirement","node_id":"req_absent"},"role":"system","body":body})
}
#[tokio::test]
async fn discussion_operations_preserve_native_results_and_persistence() {
    let native = Repository::new("The shared graph is readable.");
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let store = StateStore::new(native.layout.clone());
    let scope = ScopeId::new("default").unwrap();
    for body in ["First", " Second "] {
        let expected = store
            .post_thread_message(PostMessageInput {
                scope_id: scope.clone(),
                parent: ThreadParent {
                    node_type: NodeType::Requirement,
                    node_id: StableId::new("req_absent").unwrap(),
                },
                role: MessageRole::System,
                body: body.into(),
            })
            .unwrap();
        let (status, actual) = call(&host, "post-thread-message", scoped(&input(body))).await;
        assert_eq!(status, 200, "{actual}");
        assert_eq!(actual, serde_json::to_value(expected).unwrap());
    }
    for (operation, expected) in [
        ("list-threads", json!(store.list_threads(&scope).unwrap())),
        ("list-messages", json!(store.list_messages(&scope).unwrap())),
    ] {
        let (status, actual) = call(&host, operation, scoped(&Value::Null)).await;
        assert_eq!(status, 200, "{actual}");
        assert_eq!(actual, expected);
    }
    host.shutdown().await;
    let actual = StateStore::new(repo.layout.clone());
    assert_eq!(
        actual.list_threads(&scope).unwrap(),
        store.list_threads(&scope).unwrap()
    );
    assert_eq!(
        actual.list_messages(&scope).unwrap(),
        store.list_messages(&scope).unwrap()
    );
}

#[tokio::test]
async fn discussion_refusals_have_no_effects() {
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let scope = ScopeId::new("default").unwrap();
    let store = StateStore::new(repo.layout.clone());
    let mut mismatch = input("Text");
    mismatch["scope_id"] = json!("other");
    let mut domain = input("Text");
    domain["parent"]["node_type"] = json!("domain");
    let mut boundary = input("Text");
    boundary["parent"]["node_type"] = json!("boundary");
    for (request, kind) in [
        (input(" \n"), "empty_message_body"),
        (domain, "unsupported_thread_parent"),
        (boundary, "unsupported_thread_parent"),
        (mismatch, "scope_mismatch"),
    ] {
        let (status, actual) = call(&host, "post-thread-message", scoped(&request)).await;
        assert_eq!(status, 400, "{actual}");
        assert_eq!(actual["error"]["kind"], kind);
        assert!(store.list_threads(&scope).unwrap().is_empty());
        assert!(store.list_messages(&scope).unwrap().is_empty());
        assert!(!repo.layout.scopes_dir().join("other").exists());
    }
    host.shutdown().await;
}

#[tokio::test]
async fn message_read_failure_after_thread_publication_is_uncertain() {
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let scope = ScopeId::new("default").unwrap();
    let path = provenance_store::shards::messages_path(&repo.layout, &scope);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "invalid JSON\n").unwrap();
    let (status, actual) = call(&host, "post-thread-message", scoped(&input("Text"))).await;
    assert_eq!(status, 500, "{actual}");
    assert_eq!(actual["error"]["kind"], "uncertain_write");
    assert_eq!(
        StateStore::new(repo.layout.clone())
            .list_threads(&scope)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), "invalid JSON\n");
    host.shutdown().await;
}

#[tokio::test]
async fn mcp_discussions_use_explicit_write_grants_and_complete_native_lists() {
    use rmcp::{model::CallToolRequestParam, ServiceExt};
    let repo = Repository::new("The shared graph is readable.");
    let scope = ScopeId::new("default").unwrap();
    for writable in [false, true] {
        let host = if writable {
            writable_host(&repo)
        } else {
            records::host(&[("selected", &repo)], &["selected"])
        };
        let (client_io, server_io) = tokio::io::duplex(256 * 1024);
        let server_host = host.clone();
        let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
        let client = ().serve(client_io).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();
        assert_eq!(
            tools.iter().any(|tool| tool.name == "post-thread-message"),
            writable
        );
        for name in ["list-threads", "list-messages"] {
            assert!(tools.iter().any(|tool| tool.name == name));
        }
        let result = client
            .call_tool(CallToolRequestParam {
                name: "post-thread-message".into(),
                arguments: Some(
                    json!({"protocol_version":7,"call":scoped(&input("MCP"))})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            })
            .await
            .unwrap();
        let store = StateStore::new(repo.layout.clone());
        if writable {
            assert_ne!(result.is_error, Some(true), "{result:?}");
            let actual = result.structured_content.unwrap();
            assert_eq!(
                actual["message"],
                json!(store.list_messages(&scope).unwrap()[0])
            );
            assert_eq!(
                actual["thread"],
                json!(store.list_threads(&scope).unwrap()[0])
            );
        } else {
            assert_eq!(result.is_error, Some(true));
            assert_eq!(
                result.structured_content.unwrap()["error"]["kind"],
                "access_denied"
            );
            assert!(store.list_threads(&scope).unwrap().is_empty());
        }
        for (name, expected) in [
            ("list-threads", json!(store.list_threads(&scope).unwrap())),
            ("list-messages", json!(store.list_messages(&scope).unwrap())),
        ] {
            let result = client
                .call_tool(CallToolRequestParam {
                    name: name.into(),
                    arguments: Some(
                        json!({"protocol_version":7,"call":scoped(&Value::Null)})
                            .as_object()
                            .unwrap()
                            .clone(),
                    ),
                })
                .await
                .unwrap();
            assert_ne!(result.is_error, Some(true), "{result:?}");
            assert_eq!(
                result.structured_content.unwrap(),
                json!({"result":expected})
            );
        }
        client.cancel().await.unwrap();
        server.await.unwrap().cancel().await.unwrap();
        host.shutdown().await;
    }
}

#[tokio::test]
async fn canonical_active_selection_and_terminal_history_match_native() {
    let native = Repository::new("The shared graph is readable.");
    let repo = Repository::new("The shared graph is readable.");
    let scope = ScopeId::new("default").unwrap();
    let threads = [
        ("thread_z", "active", 1), ("thread_a", "active", 1),
        ("thread_old", "resolved", 0), ("thread_archived", "archived", 0),
    ].map(|(id, status, created_at)| json!({"schema_version":provenance_core::SUPPORTED_SCHEMA_VERSION,"scope_id":"default","id":id,
        "parent":{"node_type":"requirement","node_id":"req_absent"},"status":status,"created_at":created_at}));
    for layout in [&native.layout, &repo.layout] {
        let path = provenance_store::shards::threads_path(layout, &scope);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut rows = String::new();
        for value in &threads {
            writeln!(rows, "{value}").unwrap();
        }
        std::fs::write(path, rows).unwrap();
    }
    let host = writable_host(&repo);
    let store = StateStore::new(native.layout.clone());
    let expected = store
        .post_thread_message(PostMessageInput {
            scope_id: scope.clone(),
            parent: ThreadParent {
                node_type: NodeType::Requirement,
                node_id: StableId::new("req_absent").unwrap(),
            },
            role: MessageRole::System,
            body: "Text".into(),
        })
        .unwrap();
    let (status, result) = call(&host, "post-thread-message", scoped(&input("Text"))).await;
    assert_eq!(status, 200, "{result}");
    assert_eq!(result, json!(expected));
    assert_eq!(result["thread"]["id"], "thread_a");
    let (status, result) = call(&host, "list-threads", scoped(&Value::Null)).await;
    assert_eq!(status, 200, "{result}");
    assert_eq!(result, json!(store.list_threads(&scope).unwrap()));
    let actual = result.as_array().unwrap();
    assert_eq!(
        actual.iter().find(|row| row["id"] == "thread_old").unwrap()["status"],
        "resolved"
    );
    assert_eq!(
        actual.iter().find(|row| row["id"] == "thread_z").unwrap()["status"],
        "archived"
    );
    host.shutdown().await;
}

#[tokio::test]
async fn all_six_existing_parent_kinds_accept_absent_records() {
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    for kind in [
        "source",
        "requirement",
        "resolution",
        "rule",
        "topic",
        "question",
    ] {
        let mut request = input("Text");
        request["parent"]["node_type"] = json!(kind);
        let (status, actual) = call(&host, "post-thread-message", scoped(&request)).await;
        assert_eq!(status, 200, "{actual}");
        assert_eq!(actual["thread"]["parent"]["node_type"], kind);
    }
    host.shutdown().await;
}

#[test]
fn old_discussion_json_roundtrips_without_new_fields() {
    let thread = json!({"schema_version":1,"scope_id":"default","id":"thread_old",
        "parent":{"node_type":"rule","node_id":"rule_old"},"status":"resolved","created_at":1});
    let message = json!({"schema_version":1,"scope_id":"default","id":"msg_old",
        "thread_id":"thread_old","role":"assistant","body":"Old text","created_at":2,
        "ai_metadata":{"model":"fixture"}});
    assert_eq!(
        json!(serde_json::from_value::<provenance_core::Thread>(thread.clone()).unwrap()),
        thread
    );
    assert_eq!(
        json!(serde_json::from_value::<provenance_core::Message>(message.clone()).unwrap()),
        message
    );
    let mut legacy = message.clone();
    legacy.as_object_mut().unwrap().remove("body");
    legacy["content"] = json!("Old text");
    assert_eq!(
        json!(serde_json::from_value::<provenance_core::Message>(legacy).unwrap()),
        message
    );
    let mut without_metadata = message;
    without_metadata
        .as_object_mut()
        .unwrap()
        .remove("ai_metadata");
    assert_eq!(
        json!(
            serde_json::from_value::<provenance_core::Message>(without_metadata.clone()).unwrap()
        ),
        without_metadata
    );
}

#[test]
fn native_discussion_refusals_keep_display_text() {
    let repo = Repository::new("The shared graph is readable.");
    let store = StateStore::new(repo.layout);
    for (kind, body, message) in [
        (NodeType::Requirement, " ", "message body must not be empty"),
        (NodeType::Domain, "Text", "thread parent kind `domain` is not supported; threads attach to a source, requirement, resolution, rule, topic, or question"),
        (NodeType::Boundary, "Text", "thread parent kind `boundary` is not supported; threads attach to a source, requirement, resolution, rule, topic, or question"),
    ] {
        let error = store.post_thread_message(PostMessageInput { scope_id: ScopeId::new("default").unwrap(),
            parent: ThreadParent { node_type: kind, node_id: StableId::new("absent").unwrap() },
            role: MessageRole::User, body: body.into() }).unwrap_err();
        assert_eq!(error.to_string(), message);
    }
}
