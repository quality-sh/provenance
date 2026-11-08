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

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::{mcp::Session, records::Repository};

/// Withdraw the pending submission of the Requirement, so that a submit can run.
fn withdraw_pending_submission(repository: &Repository) {
    let store = repository.open().store();
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let requirement = provenance_core::StableId::new("req_shared").unwrap();
    let state = store
        .requirement_decision_state(&scope, &requirement)
        .unwrap();
    let proposal = state.pending.unwrap().proposal_id;
    let withdrawal = json!({"scope_id": "default", "actor": "agent", "proposal_id": proposal});
    store
        .withdraw_record_review(serde_json::from_value(withdrawal).unwrap())
        .unwrap();
}

/// One call of each change action, and the record kind of a record result.
fn calls() -> Vec<(Value, Option<&'static str>)> {
    vec![
        (
            json!({"action": "create", "target": "source_new", "type": "source",
                   "name": "New source", "source_type": "document", "supersedes": []}),
            Some("source"),
        ),
        (
            json!({"action": "answer", "target": "question_shared",
                   "answer": "The shared record applies."}),
            Some("question"),
        ),
        (
            json!({"action": "claim", "target": "topic_shared", "actor": "ben"}),
            Some("topic"),
        ),
        (
            json!({"action": "release", "target": "topic_shared", "actor": "ben"}),
            Some("topic"),
        ),
        (
            json!({"action": "submit", "target": "req_shared",
                   "actor": "ben", "title": "Shared", "summary": "Submit it.",
                            "source_ids": [], "evidence_references": [], "builds_on": []}),
            Some("requirement"),
        ),
        (
            json!({"action": "discuss",
                   "target": "req_shared",
                   "actor": "ben", "role": "user", "body": "Opening text"}),
            None,
        ),
        (
            json!({"action": "notify", "target": "req_shared",
                   "notify": "codex", "thread": "thread-1"}),
            None,
        ),
    ]
}

#[tokio::test]
#[verifies("rule_mcp_one_tool_for_record_actions", examples)]
async fn each_change_result_matches_exactly_one_output_branch() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    withdraw_pending_submission(&repository);
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
        json!({"target": "req_shared"}),
    )
    .await;
    let id = started.structured_content.unwrap()["result"]["entries"][0]["discussion_id"].clone();
    let read = support::mcp::call(
        &session.client,
        "api",
        json!({"path": "requirements/req_shared"}),
    )
    .await;
    let etag = read.structured_content.unwrap()["data"]["edit"]["etag"].clone();
    for (arguments, kind) in [
        (
            json!({"action": "reply", "target": id, "expected_version": 1,
                   "actor": "ben", "role": "user", "body": "Yes."}),
            None,
        ),
        (
            json!({"action": "update", "target": "req_shared", "if_match": etag,
                   "actor": "ben", "statement": "The shared graph is readable by all."}),
            Some("requirement"),
        ),
    ] {
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
    }
    session.close().await;
}

/// No two branches of the change output accept one result. The error branch takes
/// only `error` and `meta`, and each result branch requires `action`, so no result
/// matches the error branch. Each result branch requires `type` when it has one, and
/// each pair of action and record kind is in one branch only.
#[tokio::test]
#[verifies("rule_mcp_one_tool_for_record_actions", examples)]
async fn the_change_output_branches_have_distinct_discriminants() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let (_, output) = support::schema_check::schemas(&session.client, "change").await;
    let branches = output["oneOf"].as_array().unwrap();
    let required = |branch: &Value, name: &str| {
        branch["required"]
            .as_array()
            .is_some_and(|required| required.contains(&json!(name)))
    };
    let (errors, results): (Vec<_>, Vec<_>) = branches
        .iter()
        .partition(|branch| required(branch, "error"));
    let [error] = errors.as_slice() else {
        panic!("one error branch: {output}");
    };
    assert_eq!(error["additionalProperties"], json!(false), "{error}");
    assert_eq!(
        error["properties"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        ["error", "meta"],
        "{error}"
    );
    assert_ne!(results.len(), 0, "{output}");
    let mut seen = std::collections::BTreeSet::new();
    for branch in results {
        assert!(required(branch, "action"), "{branch}");
        let has_type = branch["properties"].get("type").is_some();
        assert_eq!(required(branch, "type"), has_type, "{branch}");
        let values = |name: &str| -> Vec<Value> {
            let property = &branch["properties"][name];
            property["enum"]
                .as_array()
                .cloned()
                .or_else(|| property.get("const").map(|value| vec![value.clone()]))
                .unwrap_or_else(|| vec![Value::Null])
        };
        for action in values("action") {
            assert!(action.is_string(), "{branch}");
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
