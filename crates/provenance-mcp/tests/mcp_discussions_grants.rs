//! The discussions tool offers only the Discussion reads that the host grants, and its
//! description is the description of the read that it runs without an action argument.
//!
//! These tests are bound to no Rule. The one Rule on the text of a tool description states
//! that the MCP tool shows the description of CLI help, and a host that grants only the
//! read of one Discussion shows the description of that read, which CLI help gives for the
//! discussion command and not for the discussions command.
#![cfg(feature = "test-fixture")]

use crate::mcp_instructions::{host_without, served_tools};
use crate::support::{
    fixture_host,
    mcp::{call, Session},
    records::Repository,
};
use provenance_store::operations::catalog::{self, Operation as _};
use serde_json::{json, Value};

/// The description of the list of Discussions.
const LIST: &str = "List addressed Discussions in a scope or under a record.";
/// The description of the read of one Discussion.
const READ: &str = "Read the addressed Discussion.";

#[tokio::test]
async fn the_discussions_tool_offers_only_the_reads_that_the_host_grants() {
    let repository = Repository::new("The shared graph is readable.");
    let hosts = [
        (
            fixture_host::full(&repository),
            LIST,
            json!(["discussions", "discussion"]),
        ),
        (
            host_without(&repository, &[catalog::ListDiscussions::NAME]),
            READ,
            json!(["discussion"]),
        ),
        (
            host_without(&repository, &[catalog::GetDiscussionConversation::NAME]),
            LIST,
            json!(["discussions"]),
        ),
    ];
    for (host, description, actions) in hosts {
        let (tools, _) = served_tools(host).await;
        let tool = tools
            .iter()
            .find(|tool| tool.name == "discussions")
            .unwrap();
        let action = &tool.input_schema["properties"]["action"];
        assert_eq!(
            (
                tool.description.as_deref(),
                &action["enum"],
                &action["default"]
            ),
            (Some(description), &actions, &actions[0]),
            "{action}"
        );
    }
}

#[tokio::test]
async fn a_call_without_an_action_on_a_host_that_grants_only_the_read_reads_the_discussion() {
    let repository = Repository::new("A requirement has discussions.");
    let session =
        Session::start(host_without(&repository, &[catalog::ListDiscussions::NAME])).await;
    let started = call(
        &session.client,
        "change",
        json!({"action":"discuss", "target":"req_shared",
            "actor":"ben", "role":"user", "body":"Opening text"}),
    )
    .await;
    assert_ne!(started.is_error, Some(true), "{started:?}");
    let id = started.structured_content.unwrap()["receipt"]["discussion_id"].clone();
    let read = call(&session.client, "discussions", json!({"target": id})).await;
    let content = read.structured_content.clone().unwrap_or(Value::Null);
    assert!(
        read.is_error != Some(true) && content.to_string().contains("Opening text"),
        "{read:?}"
    );
    session.close().await;
}
