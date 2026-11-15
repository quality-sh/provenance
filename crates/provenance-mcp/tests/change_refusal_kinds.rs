//! The change tool tells a write that the host cannot run from a write that the host
//! access denies. A write that needs a repository binding or a notify port that the host
//! does not bind answers unavailable_needs; a denied write answers access_denied.
//! No Rule binds these rows; the owner of the refusal kinds decides the Rule.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_transport::{fixture::FixtureAccess, StatementHost};
use serde_json::{json, Value};
use std::sync::Arc;
use support::{fixture_host::FixtureNotify, mcp::Session, records::Repository};

fn access(repository: &Repository) -> FixtureAccess {
    support::resource_http::access(repository).allow_writes()
}

/// The complete MCP answer to one change call, with the row name.
async fn row(name: &str, host: StatementHost, arguments: Value) -> Value {
    let session = Session::start(host).await;
    let result = support::mcp::call(&session.client, "change", arguments).await;
    session.close().await;
    json!({"row": name, "answer": result})
}

/// The answer row of one refusal with this error object and empty metadata.
fn refused(name: &str, error: &Value) -> Value {
    let content = json!({"error": error, "meta": {}});
    json!({"row": name, "answer": {
        "content": [{"type": "text", "text": content.to_string()}],
        "isError": true,
        "structuredContent": content
    }})
}

fn denied(name: &str) -> Value {
    refused(
        name,
        &json!({"kind": "access_denied", "message": "access denied"}),
    )
}

fn unavailable(name: &str) -> Value {
    refused(name, &json!({"kind": "unavailable_needs"}))
}

#[tokio::test]
async fn an_unbound_need_and_a_denied_operation_answer_different_kinds() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let rows = vec![
        row(
            "materialize on a host that binds no repository",
            StatementHost::with_fixture_access(access(&repository)),
            json!({"action": "materialize"}),
        )
        .await,
        row(
            "notify on a host that binds no notify port",
            StatementHost::with_fixture_access(access(&repository)),
            json!({"action": "notify", "target": "req_shared",
                   "notify": "codex", "thread": "thread-1"}),
        )
        .await,
        row(
            "notify on a host that denies the record read",
            StatementHost::with_fixture_access(
                access(&repository).deny_operation("resolve-record"),
            )
            .with_notify_port(Arc::new(FixtureNotify)),
            json!({"action": "notify", "target": "req_shared",
                   "notify": "codex", "thread": "thread-1"}),
        )
        .await,
        row(
            "update on a host that denies update-source",
            StatementHost::with_fixture_access(access(&repository).deny_operation("update-source")),
            json!({"action": "update", "target": "source_shared", "name": "Denied update"}),
        )
        .await,
    ];
    assert_eq!(
        Value::Array(rows),
        json!([
            unavailable("materialize on a host that binds no repository"),
            unavailable("notify on a host that binds no notify port"),
            denied("notify on a host that denies the record read"),
            denied("update on a host that denies update-source"),
        ])
    );
}
