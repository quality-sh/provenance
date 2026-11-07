//! A value that the field schema does not allow gets a refusal that names the field,
//! gives a reason, and says the allowed values or the range.
#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/check_port.rs"]
    pub mod check_port;
    pub mod fixture_host;
    pub mod mcp;
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
    pub mod schema_check;
}

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::{mcp::Session, records::Repository};

/// The error object of a refused call.
async fn refused(session: &Session, name: &str, arguments: Value) -> Value {
    let result = support::schema_check::call(&session.client, name, arguments.clone()).await;
    assert_eq!(result.is_error, Some(true), "{name}: {arguments}");
    result.structured_content.unwrap()["error"].clone()
}

/// One call, the field and reason of its refusal, and a text that its message has.
type Case = (
    &'static str,
    Value,
    &'static str,
    &'static str,
    &'static str,
);

/// The cases of the change tool.
fn change_cases() -> Vec<Case> {
    vec![
        (
            "change",
            json!({"action": "discuss", "target": "req_shared", "actor": "ben",
                   "role": "bogus", "body": "Hi."}),
            "role",
            "invalid_value",
            "user, assistant, system",
        ),
        (
            "change",
            json!({"action": "discuss", "target": "",
                   "actor": "ben", "role": "user", "body": "Hi."}),
            "target",
            "invalid_value",
            "at least 1",
        ),
        (
            "change",
            json!({"action": "reply", "target": "discussion_x", "expected_version": -1,
                   "actor": "ben", "role": "user", "body": "Hi."}),
            "expected_version",
            "invalid_value",
            "at least 0",
        ),
        (
            "change",
            json!({"action": "update", "target": "req_shared", "statement": 7}),
            "statement",
            "invalid_value",
            "string",
        ),
    ]
}

/// The cases of the read tools.
fn read_cases() -> Vec<Case> {
    vec![
        (
            "discussions",
            json!({"limit": 500}),
            "limit",
            "invalid_value",
            "from 1 to 200",
        ),
        (
            "discussions",
            json!({"status": "bogus"}),
            "status",
            "invalid_value",
            "active, resolved, all",
        ),
        (
            "discussions",
            json!({"target": ""}),
            "target",
            "invalid_value",
            "at least 1",
        ),
        (
            "search",
            json!({"bogus": 1}),
            "bogus",
            "unknown_field",
            "text",
        ),
        (
            "search",
            json!({"limit": 0}),
            "limit",
            "invalid_value",
            "from 1 to 200",
        ),
        (
            "search",
            json!({"kind": ["bogus"]}),
            "kind[0]",
            "invalid_value",
            "source, requirement",
        ),
        (
            "get",
            json!({"target": "req_shared", "view": "bogus"}),
            "view",
            "invalid_value",
            "record, children",
        ),
        ("get", json!({}), "target", "required", "target"),
        (
            "get",
            json!({"target": "req_shared", "limit": 0}),
            "limit",
            "invalid_value",
            "at least 1",
        ),
        (
            "trace",
            json!({"target": 5}),
            "target",
            "invalid_value",
            "string",
        ),
        (
            "check",
            json!({"graph": "bogus"}),
            "graph",
            "invalid_value",
            "boolean",
        ),
        (
            "check",
            json!({"categories": ["graph"]}),
            "categories",
            "unknown_field",
            "base, baseline, bindings",
        ),
        (
            "api",
            json!({"method": "delete", "path": "sources"}),
            "method",
            "invalid_value",
            "get, post, patch",
        ),
        ("api", json!({"bogus": 1}), "bogus", "unknown_field", "path"),
    ]
}

#[tokio::test]
#[verifies("rule_mcp_refusal_names_field", examples)]
async fn a_value_that_the_field_schema_does_not_allow_names_the_field_and_the_allowed_values() {
    let repository = Repository::new("A requirement has discussions.");
    repository.all_kinds();
    let session = Session::start(support::fixture_host::full(&repository)).await;
    for (name, arguments, field, reason, allowed) in change_cases().into_iter().chain(read_cases())
    {
        let error = refused(&session, name, arguments.clone()).await;
        assert_eq!(
            (&error["kind"], &error["field"], &error["reason"]),
            (&json!("invalid_input"), &json!(field), &json!(reason)),
            "{name} {arguments}: {error}"
        );
        let message = error["message"].as_str().unwrap_or_default();
        assert!(message.contains(allowed), "{name} {arguments}: {message}");
    }
    session.close().await;
}

/// An action of the change tool, called on the discussions tool, names the change tool.
#[tokio::test]
#[verifies("rule_mcp_refusal_names_field", examples)]
async fn a_write_action_on_the_discussions_tool_names_the_change_tool() {
    let repository = Repository::new("A requirement has discussions.");
    let session = Session::start(support::fixture_host::full(&repository)).await;
    for action in ["discuss", "reply"] {
        let error = refused(&session, "discussions", json!({"action": action})).await;
        assert_eq!(
            (&error["field"], &error["reason"]),
            (&json!("action"), &json!("invalid_value")),
            "{error}"
        );
        assert_eq!(
            error["message"],
            format!("{action} writes; call the change tool with action {action}")
        );
    }
    let error = refused(&session, "discussions", json!({"action": "bogus"})).await;
    assert_eq!(
        error["message"],
        "unknown action bogus; the actions are: discussions, discussion"
    );
    session.close().await;
}
