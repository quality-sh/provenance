//! A review decision by an actor that the manifest field `disposition_actor_ids`
//! does not contain gets an `access_denied` refusal that names the field and the
//! init command. The refusal gives no actor ID of the field. An init command
//! that gives the field a list without the actor of a stored decision gets its
//! own refusal.

use crate::review_host_support;

use provenance_macros::verifies;
use review_host_support::surfaces::Surface;
use review_host_support::{follow::create, repository_with_actors};
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};

/// The refusal of a decision when the field is empty.
const EMPTY_FIELD: &str = "The manifest field disposition_actor_ids is empty, thus no actor can \
    make a review decision. To change this field, use the command provenance init \
    --disposition-actor-id <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each \
    person that can make a review decision.";
/// The refusal of a decision when the field holds other actors.
const ACTOR_NOT_LISTED: &str = "The manifest field disposition_actor_ids does not contain the \
    actor of a review decision. To change this field, use the command provenance init \
    --disposition-actor-id <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each \
    person that can make a review decision.";
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

/// Checks that a decision by [`STRANGER`] through `surface` gets the refusal
/// for `actor_ids` in the field, and that the complete answer gives no actor ID.
fn refused(surface: Surface, actor_ids: &[&str]) {
    let (repo, proposal) = repository(actor_ids);
    let path = decide_path(&proposal);
    let answer = surface.call(repo.path(), "post", &path, Some(&decision(STRANGER)));
    assert_eq!(answer.code, surface.refused(403), "{}", answer.text);
    assert_eq!(answer.body, expected_refusal(actor_ids), "{}", answer.text);
    for id in LISTED.into_iter().chain([STRANGER]) {
        assert!(!answer.text.contains(id), "{id} in {}", answer.text);
    }
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn cli_api_refusal_names_the_field_when_the_field_is_empty() {
    refused(Surface::Cli, &[]);
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn cli_api_refusal_names_the_field_when_the_field_holds_other_actors() {
    refused(Surface::Cli, &LISTED);
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn http_refusal_names_the_field_when_the_field_is_empty() {
    refused(Surface::Http, &[]);
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn http_refusal_names_the_field_when_the_field_holds_other_actors() {
    refused(Surface::Http, &LISTED);
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn mcp_refusal_names_the_field_when_the_field_is_empty() {
    refused(Surface::Mcp, &[]);
}

#[test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
fn mcp_refusal_names_the_field_when_the_field_holds_other_actors() {
    refused(Surface::Mcp, &LISTED);
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
    let (command, _) = message
        .split_once("use the command provenance ")
        .and_then(|(_, rest)| rest.split_once(". "))
        .unwrap_or_else(|| panic!("no command in {message}"));
    let command = command.replace(PLACEHOLDER, STRANGER);
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
