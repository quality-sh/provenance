//! The answers of the change tool to a write that the host does not list. A repository
//! action on a host with no repository binding answers `unavailable_needs`. Notify on a
//! host that grants the record read but binds no notify port has the input checks of a
//! host with a notify port, and then goes to the shared get operation, as on the CLI.
//! Notify on a host that denies the record read is denied.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_porcelain::get::NotifyInput;
use provenance_transport::{fixture::FixtureAccess, StatementHost};
use serde_json::{json, Value};
use std::sync::Arc;
use support::{fixture_host::FixtureNotify, mcp::Session, records::Repository};

fn access(repository: &Repository) -> FixtureAccess {
    support::resource_http::access(repository).allow_writes()
}

/// The complete MCP answer to one change call.
async fn answer(host: StatementHost, arguments: Value) -> Value {
    let session = Session::start(host).await;
    let result = support::mcp::call(&session.client, "change", arguments).await;
    session.close().await;
    serde_json::to_value(result).unwrap()
}

/// The MCP answer of one refusal with this `{error, meta}` object.
fn refused(content: &Value) -> Value {
    json!({
        "content": [{"type": "text", "text": content.to_string()}],
        "isError": true,
        "structuredContent": content
    })
}

/// No Rule binds this test: the Rule for repository actions on a host with no repository
/// binding is a proposal that waits for the owner. `import` with no fields pins that the
/// answer comes before the field checks of the action.
#[tokio::test]
async fn a_repository_action_without_a_repository_is_unavailable() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();

    for action in [
        "materialize",
        "import",
        "dictionary_import",
        "swarm_backtrace_land",
    ] {
        let host = StatementHost::with_fixture_access(access(&repository));
        let answer = answer(host, json!({"action": action})).await;

        assert_eq!(
            answer,
            refused(&json!({"error": {"kind": "unavailable_needs"}, "meta": {}})),
            "{action}"
        );
    }
}

/// The answer of the shared get operation to one notify request on this host.
async fn shared(host: &StatementHost, arguments: &Value) -> Value {
    let input: NotifyInput = serde_json::from_value(arguments.clone()).unwrap();
    let error = host.porcelain().get().get(input.into()).await.unwrap_err();
    error.failure()
}

/// The change arguments of one notify call with these fields.
fn notify_call(fields: &Value) -> Value {
    let mut arguments = json!({"action": "notify"});
    arguments
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    arguments
}

/// No Rule binds this test: `rule_review_notify_cli_mcp_same` is verified through a
/// running review host, and a host with no notify port exists only for a program that
/// embeds the MCP server. This test pins the owner answer to
/// `question_review_notify_no_host_failure` for that host, and that an unknown record
/// wins over the missing notify port.
#[tokio::test]
async fn notify_without_a_notify_port_answers_as_the_shared_operation() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = || StatementHost::with_fixture_access(access(&repository));
    let request = json!({"target": "req_shared", "notify": "codex", "thread": "thread-1"});
    let unknown = json!({"target": "req_missing", "notify": "codex", "thread": "thread-1"});

    let no_host = answer(host(), notify_call(&request)).await;
    let not_found = answer(host(), notify_call(&unknown)).await;

    let no_host_failure = shared(&host(), &request).await;
    assert_eq!(
        no_host_failure["error"],
        json!({"kind": "operation_failed",
               "message": "no review host is running for this repository and scope"})
    );
    assert_eq!(no_host, refused(&no_host_failure));
    assert_eq!(
        not_found["structuredContent"]["error"]["kind"],
        "resource_not_found"
    );
    assert_eq!(not_found, refused(&shared(&host(), &unknown).await));
}

/// No Rule binds this test: `rule_mcp_refusal_names_field` is verified on a host with a
/// notify port. This test pins that a host with no notify port gives the same refusal.
#[tokio::test]
async fn notify_without_a_notify_port_names_the_refused_field() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = || StatementHost::with_fixture_access(access(&repository));
    let rows = [
        (
            "thread",
            json!({"target": "req_shared", "notify": "codex", "thread": ""}),
        ),
        (
            "notify",
            json!({"target": "req_shared", "notify": "bogus", "thread": "t"}),
        ),
        (
            "thread",
            json!({"target": "req_shared", "notify": "codex", "thread": 5}),
        ),
    ];

    for (field, fields) in rows {
        let with_port = host().with_notify_port(Arc::new(FixtureNotify));
        let listed = answer(with_port, notify_call(&fields)).await;
        let unlisted = answer(host(), notify_call(&fields)).await;

        assert_eq!(
            listed["structuredContent"]["error"]["field"], field,
            "{fields}"
        );
        assert_eq!(unlisted, listed, "{fields}");
    }
}

/// No Rule binds this test: a host that reads, does not write, and binds no notify port
/// lists no change tool, so a notify call gets the unknown tool error.
#[tokio::test]
async fn notify_on_a_read_only_host_without_a_notify_port_has_no_change_tool() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = StatementHost::with_fixture_access(support::resource_http::access(&repository));
    let session = Session::start(host).await;
    let arguments = notify_call(&json!({"target": "req_shared", "notify": "codex",
                                       "thread": "thread-1"}));

    let refused = session
        .client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("change")
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap_err();
    session.close().await;

    let rmcp::ServiceError::McpError(error) = refused else {
        panic!("{refused:?}");
    };
    assert_eq!(error.message, "Unknown tool: change");
}

/// No Rule binds this test: no Rule sets the kind of a notify call that the host access
/// denies. It pins that the denied read wins over the missing notify port.
#[tokio::test]
async fn a_denied_read_wins_over_a_missing_notify_port() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host =
        StatementHost::with_fixture_access(access(&repository).deny_operation("resolve-record"));
    let arguments = json!({"action": "notify", "target": "req_shared",
                           "notify": "codex", "thread": "thread-1"});

    let answer = answer(host, arguments).await;

    assert_eq!(
        answer,
        refused(
            &json!({"error": {"kind": "access_denied", "message": "access denied"},
                        "meta": {}})
        )
    );
}
