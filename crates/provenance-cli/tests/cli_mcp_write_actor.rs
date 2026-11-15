//! A Requirement create, a Requirement update and a discuss that give no actor record the
//! actor `authoring`, on the CLI and on a spawned `provenance mcp`. Each test asserts the
//! complete answer and then reads the stored actor back through the same interface.

use crate::cli_mcp_record_actions::Server;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

pub const STATEMENT: &str = "The system stores records.";

/// The text that the CLI gives when no review host runs for the repository `.`.
const REVIEW_MESSAGE: &str = "No review host is running. Start it with `provenance review \
     --repo . --repository-id local --scope default --detach`.";

/// The exit code, the standard output and the standard error of one CLI command that runs
/// in the repository directory.
pub fn run(repo: &Path, args: &[&str]) -> (i32, String, String) {
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// The JSON answer of one CLI command that succeeds with no text on standard error.
pub fn cli_json(repo: &Path, args: &[&str]) -> Value {
    let mut all = args.to_vec();
    all.extend(["--format", "json"]);
    let (code, stdout, stderr) = run(repo, &all);
    assert_eq!((code, stderr.as_str()), (0, ""), "{all:?}");
    serde_json::from_str(&stdout).unwrap()
}

/// A new repository with one scope.
pub fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    let args = ["init", "--scope", "default", "--path-prefix", ".", "--path", repo];
    let (code, _, stderr) = run(directory.path(), &args);
    assert_eq!(code, 0, "{stderr}");
    directory
}

/// Creates `req_a` with the actor `author` through the CLI.
pub fn requirement(repo: &Path) {
    cli_json(
        repo,
        &["req_a", "create", "--type", "requirement", "--statement", STATEMENT, "--actor", "author"],
    );
}

/// The review view of `req_a`, as the CLI reads it.
pub fn cli_review(repo: &Path) -> Value {
    cli_json(repo, &["req_a", "get", "--view", "review"])["review"].clone()
}

/// The review view of `req_a`, as MCP reads it.
pub async fn mcp_review(server: &Server) -> Value {
    let read = server.client.call("get", json!({"target":"req_a","view":"review"})).await;
    assert_ne!(read["isError"], json!(true), "{read}");
    read["structuredContent"]["review"].clone()
}

/// The data of a Requirement write answer: the stored decision and edit state of `req_a`
/// and its fields.
fn requirement_data(review: &Value, description: Option<&str>) -> Value {
    let mut data = json!({
        "decision": review["decision"], "edit": review["edit"], "id": "req_a",
        "scope_id": "default", "statement": STATEMENT, "status": "active",
    });
    if let Some(description) = description {
        data["description"] = json!(description);
    }
    data
}

/// The receipt of the one Discussion of `req_a`, built from the stored Discussion.
fn receipt(review: &Value) -> Value {
    let head = &review["discussions"]["entries"][0]["head"];
    let root = head["root_message_id"].clone();
    json!({
        "scope_id": "default", "discussion_id": head["discussion_id"],
        "parent": {"node_type": "requirement", "node_id": "req_a"},
        "thread_id": "thread_requirement_req_a", "root_message_id": root,
        "message_ids": [root], "status": "active", "version": 1, "actor": "authoring",
        "outcomes": [], "disposition_id": null,
    })
}

/// The answer of one MCP change call that succeeds.
async fn change(server: &Server, arguments: Value) -> Value {
    let answer = server.client.call("change", arguments.clone()).await;
    assert_ne!(answer["isError"], json!(true), "{arguments}: {answer}");
    answer
}

/// The text content of an MCP record write: the action, the kind, the ID and the data.
fn record_text(action: &str, data: &Value) -> Value {
    let text = format!(
        "{action} requirement req_a\n\n{}",
        serde_json::to_string_pretty(data).unwrap()
    );
    json!([{"type": "text", "text": text}])
}

#[test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
fn a_cli_requirement_create_with_no_actor_records_authoring() {
    let directory = repository();
    let repo = directory.path();
    let created = cli_json(
        repo,
        &["req_a", "create", "--type", "requirement", "--statement", STATEMENT],
    );
    let review = cli_review(repo);
    assert_eq!(review["decision"]["pending"]["actor"], "authoring");
    let mut data = requirement_data(&review, None);
    data["review_message"] = json!(REVIEW_MESSAGE);
    assert_eq!(created, json!({"data": data, "meta": {}}));
}

#[test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
fn a_cli_requirement_update_with_no_actor_records_authoring() {
    let directory = repository();
    let repo = directory.path();
    requirement(repo);
    let etag = cli_review(repo)["edit"]["etag"].as_str().unwrap().to_owned();
    let updated = cli_json(
        repo,
        &["req_a", "update", "--if-match", &etag, "--description", "Edited."],
    );
    let review = cli_review(repo);
    assert_eq!(review["decision"]["pending"]["actor"], "authoring");
    let mut data = requirement_data(&review, Some("Edited."));
    data["review_message"] = json!(REVIEW_MESSAGE);
    assert_eq!(updated, json!({"data": data, "meta": {}}));
}

#[test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
fn a_cli_discuss_with_no_actor_records_authoring() {
    let directory = repository();
    let repo = directory.path();
    requirement(repo);
    let started = cli_json(repo, &["req_a", "discuss", "--body", "A concern."]);
    let review = cli_review(repo);
    let receipt = receipt(&review);
    assert_eq!(review["discussions"]["entries"][0]["head"], receipt);
    assert_eq!(started, json!({"kind": "written", "receipt": receipt}));
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn an_mcp_requirement_create_with_no_actor_records_authoring() {
    let directory = repository();
    let server = Server::start(directory.path().to_str().unwrap()).await;
    let created = change(
        &server,
        json!({"action":"create","target":"req_a","type":"requirement","statement":STATEMENT,
            "status":"active","depends_on":[],"supersedes":[]}),
    )
    .await;
    let review = mcp_review(&server).await;
    server.close().await;
    assert_eq!(review["decision"]["pending"]["actor"], "authoring");
    let data = requirement_data(&review, None);
    assert_eq!(
        created,
        json!({
            "content": record_text("create", &data),
            "structuredContent": {"action": "create", "type": "requirement", "data": data,
                "meta": {}},
        })
    );
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn an_mcp_requirement_update_with_no_actor_records_authoring() {
    let directory = repository();
    requirement(directory.path());
    let server = Server::start(directory.path().to_str().unwrap()).await;
    let etag = mcp_review(&server).await["edit"]["etag"].clone();
    let updated = change(
        &server,
        json!({"action":"update","target":"req_a","if_match":etag,"description":"Edited."}),
    )
    .await;
    let review = mcp_review(&server).await;
    server.close().await;
    assert_eq!(review["decision"]["pending"]["actor"], "authoring");
    let data = requirement_data(&review, Some("Edited."));
    assert_eq!(
        updated,
        json!({
            "content": record_text("update", &data),
            "structuredContent": {"action": "update", "type": "requirement", "data": data,
                "meta": {}},
        })
    );
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn an_mcp_discuss_with_no_actor_records_authoring() {
    let directory = repository();
    requirement(directory.path());
    let server = Server::start(directory.path().to_str().unwrap()).await;
    let started = change(
        &server,
        json!({"action":"discuss","target":"req_a","role":"user","body":"A concern."}),
    )
    .await;
    let review = mcp_review(&server).await;
    server.close().await;
    let receipt = receipt(&review);
    assert_eq!(review["discussions"]["entries"][0]["head"], receipt);
    let text = format!(
        "discussion {} parent=requirement req_a status=active version=1 message={}",
        receipt["discussion_id"].as_str().unwrap(),
        receipt["root_message_id"].as_str().unwrap()
    );
    assert_eq!(
        started,
        json!({
            "content": [{"type": "text", "text": text}],
            "structuredContent": {"action": "discuss", "kind": "written", "receipt": receipt},
        })
    );
}
