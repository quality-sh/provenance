//! A review decision by an actor that the manifest field `disposition_actor_ids`
//! does not contain gets an `access_denied` refusal that names the field and the
//! init command. The refusal gives no actor ID of the field.

#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use provenance_mcp::Client;
use review_host_support::{follow::create, repository_with_actors, request, response, start};
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

const FIELD: &str = "disposition_actor_ids";
const INIT_COMMAND: &str = "provenance init --disposition-actor-id <DISPOSITION_ACTOR_ID>";
/// The two actor IDs that the field holds in the case of other actors.
const LISTED: [&str; 2] = ["lead_reviewer_one", "lead_reviewer_two"];
const STRANGER: &str = "stranger";

/// A repository whose field holds `actor_ids`, with one pending review submission.
fn repository(actor_ids: &[&str]) -> (tempfile::TempDir, String) {
    let repo = repository_with_actors(actor_ids);
    create(repo.path(), "req_review", "requirement", &[]);
    let read = api(repo.path(), "get", "requirements/req_review", None);
    assert!(read.status.success(), "{}", stderr(&read));
    let read: Value = serde_json::from_slice(&read.stdout).unwrap();
    let proposal = read["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    (repo, proposal)
}

fn decide_path(proposal: &str) -> String {
    format!("requirements/req_review/submissions/{proposal}/decide")
}

fn decision(actor_id: &str) -> Value {
    json!({
        "actor": {"identity_type": "human", "id": actor_id},
        "decision": "rejected", "rationale": "The statement needs a scope.",
        "canonical_artifact": null, "feedback": null, "declared_by": null
    })
}

fn api(repo: &Path, method: &str, path: &str, input: Option<&Value>) -> std::process::Output {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.args(["api", path, "--repo", repo.to_str().unwrap()]);
    command.args(["--format", "json", "--method", method]);
    if input.is_some() {
        command.args(["--input", "-"]);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    if let Some(input) = input {
        std::io::Write::write_all(&mut stdin, input.to_string().as_bytes()).unwrap();
    }
    drop(stdin);
    child.wait_with_output().unwrap()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Checks the refusal `error` and that the complete answer gives no actor ID.
fn assert_guidance(error: &Value, answer: &str) {
    assert_eq!(error["kind"], "access_denied", "{answer}");
    let message = error["message"].as_str().unwrap_or_default();
    assert!(message.contains(FIELD), "{answer}");
    assert!(message.contains(INIT_COMMAND), "{answer}");
    for id in LISTED.into_iter().chain([STRANGER]) {
        assert!(!answer.contains(id), "{id} in {answer}");
    }
}

fn refused_by_cli(actor_ids: &[&str]) {
    let (repo, proposal) = repository(actor_ids);
    let refused = api(repo.path(), "post", &decide_path(&proposal), Some(&decision(STRANGER)));
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    assert!(refused.stdout.is_empty(), "{refused:?}");
    let answer = stderr(&refused);
    let envelope: Value = serde_json::from_str(answer.trim().strip_prefix("Error: ").unwrap())
        .unwrap_or_else(|error| panic!("{error}: {answer}"));
    assert_guidance(&envelope["error"], &answer);
}

fn refused_by_http(actor_ids: &[&str]) {
    let (repo, proposal) = repository(actor_ids);
    let host = start(repo.path());
    let refused = response(
        request(&host, "POST", &format!("/{}", decide_path(&proposal)), true)
            .set("Content-Type", "application/json")
            .send_string(&json!({"data": decision(STRANGER)}).to_string()),
    );
    assert_eq!(refused.status(), 403);
    let answer = refused.into_string().unwrap();
    let envelope: Value = serde_json::from_str(&answer).unwrap();
    assert_guidance(&envelope["error"], &answer);
}

async fn refused_by_mcp(actor_ids: &[&str]) {
    let (repo, proposal) = repository(actor_ids);
    let mut child = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["mcp", "--repo", repo.path().to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let io = tokio::io::join(child.stdout.take().unwrap(), child.stdin.take().unwrap());
    let client = Client::connect(io).await.unwrap();
    let refused = client
        .call(
            "api",
            json!({"path": decide_path(&proposal), "method": "post", "input": decision(STRANGER)}),
        )
        .await;
    assert_eq!(refused["isError"], json!(true), "{refused}");
    assert_guidance(&refused["structuredContent"]["error"], &refused.to_string());
    client.cancel().await.unwrap();
    assert!(child.wait().await.unwrap().success());
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn cli_api_refusal_names_the_field_when_the_field_is_empty() {
    refused_by_cli(&[]);
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn cli_api_refusal_names_the_field_when_the_field_holds_other_actors() {
    refused_by_cli(&LISTED);
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn http_refusal_names_the_field_when_the_field_is_empty() {
    refused_by_http(&[]);
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn http_refusal_names_the_field_when_the_field_holds_other_actors() {
    refused_by_http(&LISTED);
}

#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
async fn mcp_refusal_names_the_field_when_the_field_is_empty() {
    refused_by_mcp(&[]).await;
}

#[tokio::test(flavor = "multi_thread")]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
async fn mcp_refusal_names_the_field_when_the_field_holds_other_actors() {
    refused_by_mcp(&LISTED).await;
}

#[test]
#[verifies("rule_disposition_actor_allowlist", examples)]
fn a_decision_by_a_listed_actor_lands_after_a_refused_one() {
    let (repo, proposal) = repository(&LISTED);
    let path = decide_path(&proposal);
    let refused = api(repo.path(), "post", &path, Some(&decision(STRANGER)));
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));

    let decided = api(repo.path(), "post", &path, Some(&decision(LISTED[1])));
    assert!(decided.status.success(), "{}", stderr(&decided));
    let decided: Value = serde_json::from_slice(&decided.stdout).unwrap();
    assert_eq!(decided["data"]["fact"], "decided");
    assert_eq!(decided["data"]["actor"], LISTED[1]);
}
