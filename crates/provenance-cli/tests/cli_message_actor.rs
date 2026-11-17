//! Each Message records the actor of the write that added it. The Discussion reads, the
//! Discussion list and the receipt of a write show that actor on the CLI, on the HTTP API
//! through the api command, and on a spawned `provenance mcp`.

use crate::bounded_process::{provenance, Bounded as _};
use crate::cli_mcp_read_parity::cli_json;
use crate::cli_mcp_record_actions::Server;
use crate::cli_mcp_write_actor::{repository, requirement, run};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

/// Starts a Discussion under `req_a` with the actor `reviewer` and gives its ID.
fn start(repo: &Path) -> String {
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
    started["receipt"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Replies on the CLI with the actor `actor` and gives the JSON receipt.
fn reply(repo: &Path, id: &str, version: &str, actor: &str) -> Value {
    cli_json(
        repo,
        &[
            id,
            "reply",
            "--actor",
            actor,
            "--body",
            "A reply.",
            "--expected-version",
            version,
        ],
    )
}

/// The stored actor of each message row of the scope `default`, by message ID.
fn stored_actors(repo: &Path) -> std::collections::BTreeMap<String, Value> {
    let threads = repo.join(".provenance/state/scopes/default/threads");
    let mut actors = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(&threads).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if name == "discussions.jsonl" || name == "threads.jsonl" {
            continue;
        }
        let text = std::fs::read_to_string(threads.join(name)).unwrap();
        for line in text.lines() {
            let row: Value = serde_json::from_str(line).unwrap();
            actors.insert(row["id"].as_str().unwrap().to_owned(), row["actor"].clone());
        }
    }
    actors
}

/// The ID and the actor of each message in `entries`.
fn actors_of(entries: &Value) -> Vec<(String, Value)> {
    entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let id = entry["id"].as_str().unwrap().to_owned();
            (id, entry["actor"].clone())
        })
        .collect()
}

/// The JSON answer of one `provenance api` read of `path`.
fn api_read(repo: &Path, path: &str) -> Value {
    cli_json(repo, &["api", path])
}

/// The JSON answer of one `provenance api` POST of `body` to `path` with an If-Match header.
fn api_post(repo: &Path, path: &str, body: &Value, etag: &str) -> Value {
    let output = provenance()
        .args(["api", path, "--repo", repo.to_str().unwrap()])
        .args(["--method", "post", "--input", "-"])
        .args(["--header", &format!("If-Match: {etag}")])
        .write_stdin(body.to_string())
        .bounded_output();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[tokio::test]
#[verifies("rule_message_records_its_actor", examples)]
async fn a_reply_by_a_second_actor_stores_and_shows_that_actor() {
    let directory = repository();
    let repo = directory.path();
    requirement(repo);
    let id = start(repo);
    reply(repo, &id, "1", "agent-b");

    let read = cli_json(repo, &[id.as_str(), "discussion"]);
    let ids = read["result"]["head"]["message_ids"].as_array().unwrap();
    let (root, answer) = (
        ids[0].as_str().unwrap().to_owned(),
        ids[1].as_str().unwrap().to_owned(),
    );
    let expected = vec![
        (root.clone(), json!("reviewer")),
        (answer.clone(), json!("agent-b")),
    ];
    assert_eq!(stored_actors(repo), expected.iter().cloned().collect());
    assert_eq!(actors_of(&read["result"]["messages"]["entries"]), expected);

    let (code, text, _) = run(repo, &[id.as_str(), "discussion"]);
    assert_eq!(code, 0);
    for line in [
        format!("- message {root} role=User actor=reviewer"),
        format!("- message {answer} role=User actor=agent-b"),
    ] {
        assert!(text.lines().any(|shown| shown == line), "{line}: {text}");
    }

    let path = format!("requirements/req_a/discussions/{id}/messages");
    assert_eq!(actors_of(&api_read(repo, &path)["data"]["items"]), expected);
    let one = api_read(repo, &format!("{path}/{answer}"));
    assert_eq!(one["data"]["actor"], json!("agent-b"));

    let server = Server::start(repo.to_str().unwrap()).await;
    let by_mcp = server
        .client
        .call("discussions", json!({"action":"discussion","target":id}))
        .await;
    server.close().await;
    let entries = &by_mcp["structuredContent"]["result"]["messages"]["entries"];
    assert_eq!(actors_of(entries), expected);
}

#[tokio::test]
#[verifies("rule_discussion_receipt_shows_message_actor", examples)]
async fn the_receipt_of_a_reply_shows_the_actor_of_the_reply() {
    let directory = repository();
    let repo = directory.path();
    requirement(repo);
    let id = start(repo);

    let by_cli = reply(repo, &id, "1", "agent-b")["receipt"].clone();
    let path = format!("requirements/req_a/discussions/{id}/messages");
    let body = json!({"actor":"agent-c", "role":"user", "body":"A reply."});
    let by_api = api_post(repo, &path, &body, "\"2\"")["data"].clone();
    let server = Server::start(repo.to_str().unwrap()).await;
    let arguments = json!({"action":"reply","target":id,"actor":"agent-d","role":"user",
        "body":"A reply.","expected_version":3});
    let by_mcp = server.client.call("change", arguments).await;
    server.close().await;
    let mcp_receipt = &by_mcp["structuredContent"]["receipt"];

    let actors = stored_actors(repo);
    for (receipt, actor, version) in [
        (&by_cli, "agent-b", 2),
        (&by_api, "agent-c", 3),
        (mcp_receipt, "agent-d", 4),
    ] {
        let written = receipt["message_ids"][version - 1].as_str().unwrap();
        assert_eq!(receipt["actor"], json!("reviewer"), "{receipt}");
        assert_eq!(receipt["version"], json!(version), "{receipt}");
        assert_eq!(receipt["message"]["id"], json!(written), "{receipt}");
        assert_eq!(receipt["message"]["actor"], json!(actor), "{receipt}");
        assert_eq!(actors[written], json!(actor));
    }
    let written = mcp_receipt["message"]["id"].as_str().unwrap();
    let text = format!(
        "discussion {id} parent=requirement req_a status=active version=4 \
         message={written} actor=agent-d"
    );
    assert_eq!(by_mcp["content"], json!([{"type": "text", "text": text}]));
}

#[tokio::test]
#[verifies("rule_discussion_list_entry_shows_opening_actor", examples)]
async fn the_discussion_list_shows_the_actor_of_the_opening_message() {
    let directory = repository();
    let repo = directory.path();
    requirement(repo);
    let id = start(repo);
    reply(repo, &id, "1", "agent-b");

    let by_cli = cli_json(repo, &["discussions"]);
    assert_eq!(
        by_cli["result"]["entries"][0]["opening_actor"],
        json!("reviewer")
    );
    let (code, text, _) = run(repo, &["discussions"]);
    assert_eq!(code, 0);
    let line = format!(
        "- {id} parent=requirement req_a status=active version=2 actor=reviewer truncated=false"
    );
    assert!(text.lines().any(|shown| shown == line), "{line}: {text}");

    let server = Server::start(repo.to_str().unwrap()).await;
    let by_mcp = server
        .client
        .call("discussions", json!({"action":"discussions"}))
        .await;
    server.close().await;
    let entries = &by_mcp["structuredContent"]["result"]["entries"];
    assert_eq!(entries[0]["opening_actor"], json!("reviewer"));
}
