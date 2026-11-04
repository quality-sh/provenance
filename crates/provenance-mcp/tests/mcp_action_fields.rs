//! Each tool input is one flat object. The server runs a call whose fields agree with
//! its action and refuses each other call with an error that names the field to fix.
#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/check_port.rs"]
    pub mod check_port;
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
    pub mod schema_check;
}

use provenance_macros::verifies;
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use rmcp::ServiceExt as _;
use serde_json::{json, Value};
use std::sync::Arc;
use support::records::Repository;
use support::schema_check::{call, Client};

fn host(repository: &Repository) -> StatementHost {
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repository.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
    .allow_writes();
    StatementHost::with_fixture_access(access)
        .with_check_port(Arc::new(support::check_port::FixtureCheckPort))
}

async fn session(
    repository: &Repository,
) -> (Client, tokio::task::JoinHandle<provenance_mcp::Session>) {
    let host = host(repository);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move {
        Box::pin(provenance_mcp::serve(host, server_io))
            .await
            .unwrap()
    });
    (().serve(client_io).await.unwrap(), server)
}

/// The error object of a refused call.
async fn refused(client: &Client, name: &str, arguments: Value) -> Value {
    let result = call(client, name, arguments.clone()).await;
    assert_eq!(result.is_error, Some(true), "{name}: {arguments}");
    result.structured_content.unwrap()["error"].clone()
}

/// The field and reason of a refused call.
async fn field(client: &Client, name: &str, arguments: Value) -> (Value, Value) {
    let error = refused(client, name, arguments).await;
    assert!(error["field"].is_string(), "{error}");
    (error["field"].clone(), error["reason"].clone())
}

#[tokio::test]
#[verifies("rule_mcp_tool_inputs_are_flat_objects", examples)]
#[verifies("rule_mcp_tools_read_or_write", examples)]
async fn each_tool_input_is_a_flat_object_and_only_change_writes() {
    let repository = Repository::new("The shared graph is readable.");
    let (client, server) = session(&repository).await;
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 7);
    for tool in &tools {
        let input = json!(tool.input_schema);
        let output = json!(tool.output_schema);
        assert_eq!(input["type"], "object", "{}", tool.name);
        assert_eq!(output["type"], "object", "{}", tool.name);
        for keyword in ["oneOf", "anyOf", "allOf", "not", "if"] {
            assert!(input.get(keyword).is_none(), "{} has {keyword}", tool.name);
        }
        let annotations = json!(tool.annotations);
        if tool.name == "change" {
            assert_eq!(
                annotations,
                json!({"readOnlyHint": false, "destructiveHint": true,
                       "idempotentHint": false, "openWorldHint": true})
            );
        } else {
            assert_eq!(
                annotations,
                json!({"readOnlyHint": true, "openWorldHint": false}),
                "{}",
                tool.name
            );
        }
    }
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
#[verifies("rule_mcp_one_tool_for_discussions", examples)]
#[verifies("rule_mcp_refusal_names_field", examples)]
async fn discussion_reads_and_writes_refuse_fields_of_another_action() {
    let repository = Repository::new("A requirement has discussions.");
    let (client, server) = session(&repository).await;
    let started = call(
        &client,
        "change",
        json!({"action":"discuss",
            "parent":{"node_type":"requirement","node_id":"req_shared"},
            "actor":"ben", "role":"user", "body":"Opening text"}),
    )
    .await;
    assert_ne!(started.is_error, Some(true), "{started:?}");
    let id = started.structured_content.unwrap()["receipt"]["discussion_id"].clone();
    for (name, valid) in [
        ("discussions", json!({})),
        ("discussions", json!({"action":"discussions","limit":5})),
        (
            "discussions",
            json!({"action":"discussion","discussion_id":id}),
        ),
        (
            "change",
            json!({"action":"reply","discussion_id":id,"expected_version":1,
                          "actor":"ben","role":"user","body":"Yes."}),
        ),
    ] {
        let result = call(&client, name, valid.clone()).await;
        assert_ne!(result.is_error, Some(true), "{valid}: {result:?}");
    }
    for (name, invalid, expected) in [
        (
            "discussions",
            json!({"action":"discussion"}),
            ("discussion_id", "required"),
        ),
        (
            "discussions",
            json!({"action":"create"}),
            ("action", "invalid_value"),
        ),
        (
            "discussions",
            json!({"action":"discussions","body":"Hi."}),
            ("body", "unknown_field"),
        ),
        (
            "discussions",
            json!({"action":"reply","discussion_id":id}),
            ("action", "invalid_value"),
        ),
        (
            "change",
            json!({"action":"reply","discussion_id":id}),
            ("actor", "required"),
        ),
        (
            "change",
            json!({"action":"discuss","discussion_id":id,"actor":"ben","role":"user","body":"Hi."}),
            ("discussion_id", "unknown_field"),
        ),
    ] {
        assert_eq!(
            field(&client, name, invalid.clone()).await,
            (json!(expected.0), json!(expected.1)),
            "{invalid}"
        );
    }
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
#[verifies("rule_mcp_one_tool_for_record_actions", examples)]
#[verifies("rule_mcp_refusal_names_field", examples)]
async fn record_actions_refuse_missing_fields_and_other_record_kinds() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let (client, server) = session(&repository).await;
    let answered = call(
        &client,
        "change",
        json!({"action":"answer","target":"question_shared","data":{"answer":"Yes."}}),
    )
    .await;
    assert_ne!(answered.is_error, Some(true), "{answered:?}");
    for (invalid, expected) in [
        (json!({"target":"question_shared"}), ("action", "required")),
        (
            json!({"action":"invented","target":"question_shared"}),
            ("action", "invalid_value"),
        ),
        (
            json!({"action":"answer","data":{"answer":"Yes."}}),
            ("target", "required"),
        ),
        (
            json!({"action":"answer","target":"question_shared","type":"source","data":{}}),
            ("type", "unknown_field"),
        ),
        (
            json!({"action":"answer","target":"req_shared","data":{"answer":"Yes."}}),
            ("target", "invalid_value"),
        ),
        (
            json!({"action":"create","target":"source_x","type":"invented","data":{}}),
            ("type", "invalid_value"),
        ),
        (
            json!({"action":"update","target":"source_shared"}),
            ("data", "required"),
        ),
        (
            json!({"action":"create","target":"req_x","type":"requirement","body":"Hi."}),
            ("body", "unknown_field"),
        ),
    ] {
        assert_eq!(
            field(&client, "change", invalid.clone()).await,
            (json!(expected.0), json!(expected.1)),
            "{invalid}"
        );
    }
    let answer = refused(
        &client,
        "change",
        json!({"action":"answer","target":"req_shared","data":{"answer":"Yes."}}),
    )
    .await;
    assert_eq!(
        answer["message"],
        "the answer action takes a record of kind: question"
    );
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

/// A call without data gets the data schema of the action and record kind.
#[tokio::test]
#[verifies("rule_mcp_refusal_names_field", examples)]
async fn a_refusal_without_data_gives_the_data_schema_of_the_record_kind() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let (client, server) = session(&repository).await;
    let source = refused(
        &client,
        "change",
        json!({"action":"create","target":"source_x","type":"source"}),
    )
    .await;
    assert_eq!(source["field"], "data");
    assert_eq!(
        source["message"],
        "the create action on a source record needs data"
    );
    assert!(source["schema"]["properties"]["data"]["properties"]
        .get("source_type")
        .is_some());
    let requirement = refused(
        &client,
        "change",
        json!({"action":"update","target":"req_shared"}),
    )
    .await;
    assert!(requirement["schema"].to_string().contains("refines"));
    support::schema_check::compile(&requirement["schema"]);
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
