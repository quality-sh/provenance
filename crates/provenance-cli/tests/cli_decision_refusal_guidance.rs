//! A review decision by an actor that the manifest field `disposition_actor_ids`
//! does not contain gets an `access_denied` refusal that names the field and the
//! init command. The refusal gives no actor ID of the field. An init command
//! that gives the field a list without the actor of a stored decision gets its
//! own refusal.

use crate::review_host_support;

use provenance_macros::verifies;
use provenance_mcp::Client;
use review_host_support::{follow::create, repository_with_actors, request, response, start};
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

/// The refusal of a decision when the field is empty.
const EMPTY_FIELD: &str = "The manifest field disposition_actor_ids is empty, thus no actor can \
    make a review decision. To change this field, use the command provenance init \
    --disposition-actor-id <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each \
    actor that can make a review decision.";
/// The refusal of a decision when the field holds other actors.
const ACTOR_NOT_LISTED: &str = "The manifest field disposition_actor_ids does not contain the \
    actor of a review decision. To change this field, use the command provenance init \
    --disposition-actor-id <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each \
    actor that can make a review decision.";
/// The complete error output of an init command whose list does not contain
/// the actor of a stored decision.
const INIT_REFUSAL: &str = "Error: the planned Provenance state is not valid\n\nCaused by:\n    \
    The list that this init command gives to the manifest field disposition_actor_ids does not \
    contain the actor of a review decision in the state. Give --disposition-actor-id one time for \
    each actor of a review decision in the state, and one time for each other actor that can \
    make a review decision.\n";
/// The two actor IDs that the field holds in the case of other actors.
const LISTED: [&str; 2] = ["lead_reviewer_one", "lead_reviewer_two"];
const STRANGER: &str = "stranger";
/// The value name of the init flag in the refusal, which the caller replaces.
const PLACEHOLDER: &str = "<DISPOSITION_ACTOR_ID>";

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

/// The refusal that a decision by [`STRANGER`] gets with `actor_ids` in the field.
fn expected_refusal(actor_ids: &[&str]) -> Value {
    let message = if actor_ids.is_empty() {
        EMPTY_FIELD
    } else {
        ACTOR_NOT_LISTED
    };
    json!({"kind": "access_denied", "message": message})
}

/// Checks the complete refusal `error` and that the complete answer gives no actor ID.
fn assert_guidance(actor_ids: &[&str], error: &Value, answer: &str) {
    assert_eq!(error, &expected_refusal(actor_ids), "{answer}");
    for id in LISTED.into_iter().chain([STRANGER]) {
        assert!(!answer.contains(id), "{id} in {answer}");
    }
}

fn refused_by_cli(actor_ids: &[&str]) {
    let (repo, proposal) = repository(actor_ids);
    let refused = api(
        repo.path(),
        "post",
        &decide_path(&proposal),
        Some(&decision(STRANGER)),
    );
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    assert!(refused.stdout.is_empty(), "{refused:?}");
    let answer = stderr(&refused);
    let envelope: Value = serde_json::from_str(answer.trim().strip_prefix("Error: ").unwrap())
        .unwrap_or_else(|error| panic!("{error}: {answer}"));
    assert_guidance(actor_ids, &envelope["error"], &answer);
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
    assert_guidance(actor_ids, &envelope["error"], &answer);
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
    assert_guidance(
        actor_ids,
        &refused["structuredContent"]["error"],
        &refused.to_string(),
    );
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

/// Runs `provenance init` in `repo` with `args`.
fn init(repo: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.env(
        "PROVENANCE_TEST_STE100_ASSET_URL",
        env!("PROVENANCE_TEST_STE100_ASSET_URL"),
    );
    command.args(["init", "--path", repo.to_str().unwrap()]);
    command.args(args).output().unwrap()
}

/// A repository whose field holds [`LISTED`], with one decision by `LISTED[0]`.
fn repository_with_a_decision() -> tempfile::TempDir {
    let (repo, proposal) = repository(&LISTED);
    let decided = api(
        repo.path(),
        "post",
        &decide_path(&proposal),
        Some(&decision(LISTED[0])),
    );
    assert!(decided.status.success(), "{}", stderr(&decided));
    repo
}

/// Checks that init refuses `args` with the init refusal and changes no manifest.
fn assert_init_refused(args: &[&str]) {
    let repo = repository_with_a_decision();
    let manifest = repo.path().join(".provenance/state/manifest.json");
    let before = std::fs::read(&manifest).unwrap();
    let refused = init(repo.path(), args);
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    assert_eq!(stderr(&refused), INIT_REFUSAL);
    for id in LISTED.into_iter().chain(["maintainer"]) {
        assert!(!stderr(&refused).contains(id), "{id}");
    }
    assert_eq!(std::fs::read(&manifest).unwrap(), before);
}

#[test]
#[verifies("rule_init_validates_planned_repository", examples)]
fn init_refuses_a_list_without_the_actor_of_a_stored_decision() {
    assert_init_refused(&["--disposition-actor-id", "maintainer"]);
}

#[test]
#[verifies("rule_init_validates_planned_repository", examples)]
fn init_refuses_to_clear_the_list_when_the_state_holds_a_decision() {
    assert_init_refused(&["--clear-disposition-actors"]);
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
fn the_init_command_in_the_refusal_lets_the_same_decision_land() {
    let (repo, proposal) = repository(&[]);
    let path = decide_path(&proposal);
    let refused = api(repo.path(), "post", &path, Some(&decision(STRANGER)));
    assert_eq!(refused.status.code(), Some(1), "{}", stderr(&refused));
    let answer = stderr(&refused);
    let envelope: Value = serde_json::from_str(answer.trim().strip_prefix("Error: ").unwrap())
        .unwrap_or_else(|error| panic!("{error}: {answer}"));
    let message = envelope["error"]["message"].as_str().unwrap();
    let command = message
        .split_once("use the command provenance ")
        .and_then(|(_, rest)| rest.split_once(". "))
        .map(|(command, _)| command.replace(PLACEHOLDER, STRANGER))
        .unwrap_or_else(|| panic!("no command in {message}"));
    let (subcommand, args) = command.split_once(' ').unwrap();
    assert_eq!(subcommand, "init", "{message}");
    let args: Vec<&str> = args.split_whitespace().collect();
    let changed = init(repo.path(), &args);
    assert!(changed.status.success(), "{}", stderr(&changed));

    let decided = api(repo.path(), "post", &path, Some(&decision(STRANGER)));
    assert!(decided.status.success(), "{}", stderr(&decided));
    let decided: Value = serde_json::from_slice(&decided.stdout).unwrap();
    assert_eq!(decided["data"]["fact"], "decided");
    assert_eq!(decided["data"]["actor"], STRANGER);
}
