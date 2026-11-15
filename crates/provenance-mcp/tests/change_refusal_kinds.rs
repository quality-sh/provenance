//! The answers of the change tool to a write that the host does not list. A repository
//! action on a host with no repository binding answers `unavailable_needs`. Notify on a
//! host that grants the record read but binds no notify port goes to the shared get
//! operation, as on the CLI. Notify on a host that denies the record read is denied.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_porcelain::get::NotifyInput;
use provenance_transport::{fixture::FixtureAccess, StatementHost};
use serde_json::{json, Value};
use support::{mcp::Session, records::Repository};

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
/// binding is a proposal that waits for the owner.
#[tokio::test]
async fn a_repository_action_without_a_repository_is_unavailable() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = StatementHost::with_fixture_access(access(&repository));

    let answer = answer(host, json!({"action": "materialize"})).await;

    assert_eq!(
        answer,
        refused(&json!({"error": {"kind": "unavailable_needs"}, "meta": {}}))
    );
}

/// The answer of the shared get operation to one notify request on this host.
async fn shared(host: &StatementHost, arguments: &Value) -> Value {
    let input: NotifyInput = serde_json::from_value(arguments.clone()).unwrap();
    let error = host.porcelain().get().get(input.into()).await.unwrap_err();
    error.failure()
}

/// No Rule binds this test: `rule_review_notify_cli_mcp_same` is verified through a
/// running review host, and a host with no notify port exists only for a program that
/// embeds the MCP server. This test pins the owner answer to
/// `question_review_notify_no_host_failure` for that host.
#[tokio::test]
async fn notify_without_a_notify_port_answers_as_the_shared_operation() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = || StatementHost::with_fixture_access(access(&repository));
    let request = json!({"target": "req_shared", "notify": "codex", "thread": "thread-1"});
    let bad_thread = json!({"target": "req_shared", "notify": "codex", "thread": ""});
    let call = |fields: &Value| {
        let mut arguments = json!({"action": "notify"});
        arguments
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        arguments
    };

    let no_host = answer(host(), call(&request)).await;
    let invalid = answer(host(), call(&bad_thread)).await;

    let no_host_failure = shared(&host(), &request).await;
    assert_eq!(
        no_host_failure["error"],
        json!({"kind": "operation_failed",
               "message": "no review host is running for this repository and scope"})
    );
    assert_eq!(no_host, refused(&no_host_failure));
    let invalid_failure = shared(&host(), &bad_thread).await;
    assert_eq!(invalid_failure["error"]["kind"], "invalid_input");
    assert_eq!(invalid, refused(&invalid_failure));
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
