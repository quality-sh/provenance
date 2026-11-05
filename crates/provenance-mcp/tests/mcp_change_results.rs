//! Each result of the change tool names its action, and a record result also names
//! the record kind, so each result matches exactly one branch of the output `oneOf`.
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

use serde_json::{json, Value};
use support::{mcp::Session, records::Repository};

/// One call of each change action, and the record kind of a record result.
fn calls() -> Vec<(Value, Option<&'static str>)> {
    vec![
        (
            json!({"action": "create", "target": "source_new", "type": "source",
                   "data": {"name": "New source", "source_type": "document"}}),
            Some("source"),
        ),
        (
            json!({"action": "create", "target": "domain_new", "type": "domain",
                   "data": {"name": "New domain"}}),
            Some("domain"),
        ),
        (
            json!({"action": "update", "target": "topic_shared",
                   "data": {"title": "Shared topic, renamed"}}),
            Some("topic"),
        ),
        (
            json!({"action": "answer", "target": "question_shared",
                   "data": {"answer": "The shared record applies."}}),
            Some("question"),
        ),
        (
            json!({"action": "claim", "target": "question_shared",
                   "data": {"holder": "ben"}}),
            Some("question"),
        ),
        (
            json!({"action": "release", "target": "question_shared",
                   "data": {"holder": "ben"}}),
            Some("question"),
        ),
        (
            json!({"action": "submit", "target": "req_shared", "data": {"actor": "ben"}}),
            Some("requirement"),
        ),
        (
            json!({"action": "discuss",
                   "parent": {"node_type": "requirement", "node_id": "req_shared"},
                   "actor": "ben", "role": "user", "body": "Opening text"}),
            None,
        ),
        (
            json!({"action": "notify", "target": "req_shared",
                   "notify": {"client": "codex", "thread": "thread-1"}}),
            None,
        ),
        (
            json!({"action": "api", "request": {"method": "post", "path": "sources",
                   "body": {"id": "source_api", "name": "Api source",
                            "source_type": "document"}}}),
            None,
        ),
    ]
}

#[tokio::test]
async fn each_change_result_matches_exactly_one_output_branch() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let (_, output) = support::schema_check::schemas(&session.client, "change").await;
    let text = output.to_string();
    assert!(!text.contains("anyOf\":[{\"additionalProperties\":false,\"properties\":{\"data\""));
    for (arguments, kind) in calls() {
        let result =
            support::schema_check::call(&session.client, "change", arguments.clone()).await;
        assert_ne!(result.is_error, Some(true), "{arguments}: {result:?}");
        let value = result.structured_content.unwrap();
        assert_eq!(value["action"], arguments["action"], "{value}");
        assert_eq!(
            value.get("type"),
            kind.map(|kind| json!(kind)).as_ref(),
            "{value}"
        );
        assert_eq!(
            support::schema_check::branches_matching(&output, &value),
            1,
            "{arguments}"
        );
    }
    let started = support::mcp::call(
        &session.client,
        "discussions",
        json!({"parent": {"node_type": "requirement", "node_id": "req_shared"}}),
    )
    .await;
    let id = started.structured_content.unwrap()["result"]["entries"][0]["discussion_id"].clone();
    let reply = support::schema_check::call(
        &session.client,
        "change",
        json!({"action": "reply", "discussion_id": id, "expected_version": 1,
               "actor": "ben", "role": "user", "body": "Yes."}),
    )
    .await;
    assert_eq!(reply.structured_content.unwrap()["action"], "reply");
    session.close().await;
}

/// No two branches of the change output accept one result: each branch has an action
/// and a record kind that no other branch has.
#[tokio::test]
async fn the_change_output_branches_have_distinct_discriminants() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let (_, output) = support::schema_check::schemas(&session.client, "change").await;
    let branches = output["oneOf"].as_array().unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for branch in &branches[..branches.len() - 1] {
        let values = |name: &str| -> Vec<Value> {
            let property = &branch["properties"][name];
            property["enum"]
                .as_array()
                .cloned()
                .or_else(|| property.get("const").map(|value| vec![value.clone()]))
                .unwrap_or_else(|| vec![Value::Null])
        };
        assert!(branch["required"]
            .as_array()
            .unwrap()
            .contains(&json!("action")));
        for action in values("action") {
            for kind in values("type") {
                assert!(
                    seen.insert((action.to_string(), kind.to_string())),
                    "{action} {kind}"
                );
            }
        }
    }
    session.close().await;
}
