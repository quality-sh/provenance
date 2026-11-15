//! The optional actor input on the CLI and on a spawned `provenance mcp`: a reply with no
//! actor is accepted and stores no actor, a JSON `null` actor is refused, and the input has
//! one description.

use crate::cli_mcp_record_actions::Server;
use crate::cli_mcp_write_actor::{cli_json, mcp_review, repository, requirement};
use serde_json::{json, Value};
use std::path::Path;

/// The description of the optional actor input.
const ACTOR_DESCRIPTION: &str = "The actor is optional for a write that makes or changes a \
     Requirement, and for a Discussion start or reply. A Requirement write or a Discussion \
     start with no actor records the actor `authoring`.";

/// The stored rows of the Discussion files of the scope `default`: the Discussion rows and
/// the message rows.
fn discussion_rows(repo: &Path) -> (Vec<Value>, Vec<Value>) {
    let threads = repo.join(".provenance/state/scopes/default/threads");
    let rows = |name: &str| {
        std::fs::read_to_string(threads.join(name))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>()
    };
    let mut messages = Vec::new();
    for entry in std::fs::read_dir(&threads).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if name != "discussions.jsonl" && name != "threads.jsonl" {
            messages.extend(rows(&name));
        }
    }
    (rows("discussions.jsonl"), messages)
}

/// The answer of one MCP change call that the server refuses.
async fn refusal(server: &Server, arguments: Value) -> Value {
    let answer = server.client.call("change", arguments.clone()).await;
    assert_eq!(answer["isError"], json!(true), "{arguments}: {answer}");
    answer["structuredContent"].clone()
}

/// Implementation aid: no Rule states what a reply stores for its actor. A reply with no
/// actor is accepted on the CLI and on MCP. The message rows have no actor, and the
/// Discussion keeps the actor of its start.
#[tokio::test]
async fn a_reply_with_no_actor_is_accepted_on_the_cli_and_mcp_and_stores_no_actor() {
    let directory = repository();
    let repo = directory.path();
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
    let id = started["receipt"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let replied = cli_json(
        repo,
        &[
            id.as_str(),
            "reply",
            "--body",
            "A reply.",
            "--expected-version",
            "1",
        ],
    );
    assert_eq!(
        (&replied["kind"], &replied["receipt"]["actor"]),
        (&json!("written"), &json!("reviewer"))
    );
    let server = Server::start(repo.to_str().unwrap()).await;
    let arguments = json!({"action":"reply","target":id,"role":"user","body":"A reply.",
        "expected_version":2});
    let replied = server.client.call("change", arguments).await;
    server.close().await;
    assert_ne!(replied["isError"], json!(true), "{replied}");
    let (discussions, messages) = discussion_rows(repo);
    assert_eq!(discussions.len(), 1);
    assert_eq!(
        (&discussions[0]["actor"], &discussions[0]["version"]),
        (&json!("reviewer"), &json!(3))
    );
    let message_ids = messages
        .iter()
        .map(|row| row["id"].clone())
        .collect::<Vec<_>>();
    assert_eq!(discussions[0]["message_ids"], json!(message_ids));
    let thread = &discussions[0]["thread_id"];
    for (row, body) in messages.iter().zip(["A concern.", "A reply.", "A reply."]) {
        let expected = json!({
            "scope_id": "default", "id": row["id"], "thread_id": thread, "role": "user",
            "body": body, "created_at": row["created_at"],
        });
        assert_eq!(row, &expected);
    }
}

/// Implementation aid: a JSON `null` actor is refused on each MCP write that takes an
/// optional actor, because the input schema allows no `null`, and the write changes no record.
#[tokio::test]
async fn a_null_actor_is_refused_on_each_mcp_write() {
    let directory = repository();
    let repo = directory.path();
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
    let id = started["receipt"]["discussion_id"].clone();
    let server = Server::start(repo.to_str().unwrap()).await;
    let before = mcp_review(&server).await;
    let writes = [
        json!({"action":"create","target":"req_b","type":"requirement","actor":null,
            "statement":"The system stores rules.","status":"active","depends_on":[],
            "supersedes":[]}),
        json!({"action":"update","target":"req_a","actor":null,
            "if_match":before["edit"]["etag"],"description":"Edited."}),
        json!({"action":"discuss","target":"req_a","actor":null,"role":"user",
            "body":"A concern."}),
        json!({"action":"reply","target":id,"actor":null,"role":"user","body":"A reply.",
            "expected_version":1}),
    ];
    let mut refusals = Vec::new();
    for arguments in writes {
        refusals.push(refusal(&server, arguments).await);
    }
    let after = mcp_review(&server).await;
    server.close().await;
    let refused = json!({"error":{"kind":"invalid_input", "field":"actor",
        "reason":"invalid_value", "message":"actor must be a string"}, "meta":{}});
    assert_eq!(refusals, vec![refused; 4]);
    assert_eq!(after, before);
    let requirements = repo.join(".provenance/state/scopes/default/requirements/req.jsonl");
    let requirements = std::fs::read_to_string(requirements).unwrap();
    assert!(!requirements.contains("req_b"), "{requirements}");
    let (discussions, messages) = discussion_rows(repo);
    assert_eq!(
        (
            discussions.len(),
            &discussions[0]["version"],
            messages.len()
        ),
        (1, &json!(1), 1)
    );
}

/// Implementation aid: a pin of one text, not evidence of a recorded actor. The optional
/// actor input has one description in the API catalog routes and in the MCP change tool, and
/// the API catalog does not require it.
#[tokio::test]
async fn the_actor_input_has_one_description_in_the_api_catalog_and_mcp() {
    let directory = repository();
    let repo = directory.path();
    for route in [
        "POST /requirements",
        "PATCH /requirements/{id}",
        "POST /requirements/{id}/discussions",
        "POST /requirements/{id}/discussions/{discussion_id}/messages",
    ] {
        let catalog = cli_json(repo, &["api", "--filter", route]);
        let request = &catalog["routes"][0]["request_schema"];
        assert_eq!(
            request["properties"]["actor"],
            json!({"description": ACTOR_DESCRIPTION, "type": "string"}),
            "{route}"
        );
        let required = request["required"].as_array().unwrap();
        assert!(!required.contains(&json!("actor")), "{route}");
    }
    let server = Server::start(repo.to_str().unwrap()).await;
    let tools = server.client.tools().await;
    server.close().await;
    let change = tools.iter().find(|tool| tool["name"] == "change").unwrap();
    let actions = "Actions: create, update, claim, release, submit, discuss, reply.";
    assert_eq!(
        change["inputSchema"]["properties"]["actor"],
        json!({"description": format!("{ACTOR_DESCRIPTION} {actions}"), "type": "string"})
    );
}
