//! Each Message records the actor of the write that added it, and each read that shows a
//! Message shows that actor: on the CLI, on the HTTP API through the api command, and on a
//! spawned `provenance mcp`.

use crate::cli_mcp_optional_actor::discussion_rows;
use crate::cli_mcp_read_parity::cli_json;
use crate::cli_mcp_record_actions::Server;
use crate::cli_mcp_write_actor::{repository, requirement, run};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

/// Starts a Discussion under `req_a` with the actor `reviewer`, replies to it with the
/// actor `agent-b`, and gives the Discussion ID and the IDs of the root Message and the
/// reply.
pub fn discussion_with_reply(repo: &Path) -> (String, String, String) {
    requirement(repo);
    let started = cli_json(
        repo,
        &[
            "req_a",
            "discuss",
            "--actor",
            "reviewer",
            "--body",
            "A concern.",
        ],
    );
    let id = started["receipt"]["discussion_id"].as_str().unwrap();
    let replied = cli_json(
        repo,
        &[
            id,
            "reply",
            "--actor",
            "agent-b",
            "--body",
            "A reply.",
            "--expected-version",
            "1",
        ],
    );
    let ids = &replied["receipt"]["message_ids"];
    let text = |value: &Value| value.as_str().unwrap().to_owned();
    (id.to_owned(), text(&ids[0]), text(&ids[1]))
}

/// The ID and the actor of each Message object in `value`, at any depth, in order.
fn actors_of(value: &Value) -> Vec<(String, Value)> {
    let mut found = Vec::new();
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            Value::Array(items) => stack.extend(items.iter().rev()),
            Value::Object(fields) => {
                if fields.contains_key("thread_id") && fields.contains_key("role") {
                    let id = fields["id"].as_str().unwrap().to_owned();
                    found.push((id, fields.get("actor").cloned().unwrap_or(Value::Null)));
                } else {
                    let mut members: Vec<&Value> = fields.values().collect();
                    members.reverse();
                    stack.extend(members);
                }
            }
            _ => {}
        }
    }
    found
}

/// The ID and the actor of each Message, sorted by ID.
fn sorted(mut actors: Vec<(String, Value)>) -> Vec<(String, Value)> {
    actors.sort_by(|left, right| left.0.cmp(&right.0));
    actors
}

#[test]
#[verifies("rule_message_records_its_actor", examples)]
fn a_reply_by_a_second_actor_stores_that_actor_on_its_message_row() {
    let directory = repository();
    let repo = directory.path();
    let (_, root, answer) = discussion_with_reply(repo);

    let (discussions, messages) = discussion_rows(repo);
    assert_eq!(discussions[0]["actor"], json!("reviewer"));
    let stored = sorted(actors_of(&json!(messages)));
    let expected = sorted(vec![(root, json!("reviewer")), (answer, json!("agent-b"))]);
    assert_eq!(stored, expected);
}

#[tokio::test]
#[verifies("rule_message_read_shows_its_actor", examples)]
async fn each_read_that_shows_a_message_shows_its_actor() {
    let directory = repository();
    let repo = directory.path();
    let (id, root, answer) = discussion_with_reply(repo);
    let expected = vec![
        (root.clone(), json!("reviewer")),
        (answer.clone(), json!("agent-b")),
    ];

    let read = cli_json(repo, &[id.as_str(), "discussion"]);
    assert_eq!(actors_of(&read["result"]["messages"]), expected);
    let (code, text, _) = run(repo, &[id.as_str(), "discussion"]);
    assert_eq!(code, 0);
    for line in [
        format!("- message {root} role=User actor=reviewer"),
        format!("- message {answer} role=User actor=agent-b"),
    ] {
        assert!(text.lines().any(|shown| shown == line), "{line}: {text}");
    }

    let review = cli_json(repo, &["req_a", "get", "--view", "review"]);
    assert_eq!(
        sorted(actors_of(&review["review"])),
        sorted(expected.clone())
    );

    let path = format!("requirements/req_a/discussions/{id}/messages");
    let page = cli_json(repo, &["api", &path]);
    assert_eq!(actors_of(&page["data"]), expected);
    let one = cli_json(repo, &["api", &format!("{path}/{answer}")]);
    assert_eq!(actors_of(&one["data"]), expected[1..]);
    let all = cli_json(repo, &["api", "messages"]);
    assert_eq!(sorted(actors_of(&all["data"])), sorted(expected.clone()));
    let one = cli_json(repo, &["api", &format!("messages/{root}")]);
    assert_eq!(actors_of(&one["data"]), expected[..1]);

    let server = Server::start(repo.to_str().unwrap()).await;
    let by_mcp = server
        .client
        .call("discussions", json!({"action":"discussion","target":id}))
        .await;
    server.close().await;
    let messages = &by_mcp["structuredContent"]["result"]["messages"];
    assert_eq!(actors_of(messages), expected);
}

#[tokio::test]
#[verifies("rule_discussion_list_entry_shows_opening_actor", examples)]
async fn the_discussion_list_shows_the_actor_of_the_opening_message() {
    let directory = repository();
    let repo = directory.path();
    let (id, _, _) = discussion_with_reply(repo);

    let by_cli = cli_json(repo, &["discussions"]);
    assert_eq!(by_cli["result"]["entries"][0]["actor"], json!("reviewer"));
    let (code, text, _) = run(repo, &["discussions"]);
    assert_eq!(code, 0);
    let line = format!(
        "- {id} parent=requirement req_a status=active version=2 truncated=false actor=reviewer"
    );
    assert!(text.lines().any(|shown| shown == line), "{line}: {text}");

    let server = Server::start(repo.to_str().unwrap()).await;
    let by_mcp = server
        .client
        .call("discussions", json!({"action":"discussions"}))
        .await;
    server.close().await;
    let entries = &by_mcp["structuredContent"]["result"]["entries"];
    assert_eq!(entries[0]["actor"], json!("reviewer"));
}
