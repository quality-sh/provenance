//! An MCP refusal with reason `unknown_field` names the fields that the call
//! takes in its message.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::{mcp::Session, records::Repository, schema_check};

/// The error object of a refused call.
async fn refused(session: &Session, name: &str, arguments: Value) -> Value {
    let result = schema_check::call(&session.client, name, arguments.clone()).await;
    assert_eq!(result.is_error, Some(true), "{name}: {arguments}");
    result.structured_content.unwrap()["error"].clone()
}

/// The property names of `schema`, in its order, joined with a comma.
fn takes(schema: &Value) -> String {
    schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ")
}

#[tokio::test]
#[verifies("rule_unknown_request_field_names_accepted_fields", examples)]
async fn a_change_field_that_the_record_kind_does_not_take_names_the_fields_it_takes() {
    let repository = Repository::new("The shared graph is readable.");
    let session = Session::start(support::fixture_host::full(&repository)).await;
    for (arguments, on, field) in [
        (
            json!({"action":"create", "target":"req_new", "type":"requirement",
                "statement":"The system records each topic.", "title":"Topic"}),
            "the create action on a requirement record",
            "title",
        ),
        (
            json!({"action":"update", "target":"rule_shared", "actor":"ben",
                "statement":"The system records each topic."}),
            "the update action on a rule record",
            "actor",
        ),
    ] {
        let error = refused(&session, "change", arguments).await;
        assert_eq!(
            (&error["kind"], &error["field"], &error["reason"]),
            (
                &json!("invalid_input"),
                &json!(field),
                &json!("unknown_field")
            ),
            "{error}"
        );
        assert_eq!(
            error["message"],
            format!(
                "{on} does not take {field}; it takes: {}",
                takes(&error["schema"])
            ),
            "{error}"
        );
    }
    session.close().await;
}

#[tokio::test]
#[verifies("rule_unknown_request_field_names_accepted_fields", examples)]
async fn get_with_notify_names_the_fields_that_get_takes() {
    let repository = Repository::new("The shared graph is readable.");
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let (input, _) = schema_check::schemas(&session.client, "get").await;
    let error = refused(
        &session,
        "get",
        json!({"target": "req_shared", "notify": {"client": "codex", "thread": "thread-1"}}),
    )
    .await;
    assert_eq!(
        error,
        json!({"kind":"invalid_input", "field":"notify", "reason":"unknown_field",
        "message":format!(
            "get does not take notify; it takes: {}; register a client thread with the \
             change tool, action notify",
            takes(&input)
        )})
    );
    session.close().await;
}
