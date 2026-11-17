//! A refusal that the CLI gives after the parser accepted the command line is
//! an operation refusal: with `--format json` it is one `{error,meta}` object
//! with exit code 1. Text mode keeps its wording and its exit code.

use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use serde_json::{json, Value};

use crate::support;
use support::review_paths::{create_requirement, provenance};

fn init() -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    (directory, repo)
}

fn command(repo: &str, args: &[&str]) -> Command {
    let mut command = provenance();
    command
        .env_remove("CODEX_THREAD_ID")
        .args(args)
        .args(["--repo", repo]);
    command
}

/// The one object that a refused command prints with `--format json`, with
/// exit code 1 and nothing on standard output.
fn refusal(mut command: Command) -> Value {
    let output = command.args(["--format", "json"]).output().unwrap();
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(output.stdout.is_empty(), "{text}");
    assert_eq!(text.lines().count(), 1, "{text}");
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

fn invalid_input(field: Option<&str>, reason: &str, message: &str) -> Value {
    json!({
        "error": {"kind": "invalid_input", "field": field, "reason": reason, "message": message},
        "meta": {}
    })
}

#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn notify_with_no_thread_is_an_invalid_input_object() {
    let (_directory, repo) = init();
    let line = ["req_a", "get", "--view", "review", "--notify", "codex"];
    let message = "--notify needs --thread or the CODEX_THREAD_ID environment variable";

    assert_eq!(
        refusal(command(&repo, &line)),
        invalid_input(Some("thread"), "required", message)
    );
    command(&repo, &line)
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty())
        .stderr(format!("Error: {message}\n"));
}

#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn standard_input_that_is_not_json_is_an_invalid_input_object() {
    let (_directory, repo) = init();
    let line = ["req_x", "create", "--type", "requirement", "--stdin"];
    let message = "expected ident at line 1 column 2";

    let mut json = command(&repo, &line);
    json.write_stdin("not json\n");
    assert_eq!(refusal(json), invalid_input(None, "invalid_value", message));
    command(&repo, &line)
        .write_stdin("not json\n")
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(format!("error: {message}\n"));
}

#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn an_option_that_the_action_does_not_take_is_an_invalid_input_object() {
    let (_directory, repo) = init();
    let line = [
        "req_a",
        "get",
        "--statement",
        "The system records one topic.",
    ];
    let message = "unsupported option --statement";

    assert_eq!(
        refusal(command(&repo, &line)),
        invalid_input(None, "invalid_value", message)
    );
    command(&repo, &line)
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(format!("error: {message}\n"));
}

#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn create_with_no_type_is_an_invalid_input_object_that_names_the_type() {
    let (_directory, repo) = init();
    let line = [
        "req_x",
        "create",
        "--statement",
        "The system records one topic.",
    ];
    let message = "create requires --type and existing-record actions infer it without --type";

    assert_eq!(
        refusal(command(&repo, &line)),
        invalid_input(Some("type"), "invalid_value", message)
    );
    command(&repo, &line)
        .assert()
        .code(2)
        .stderr(format!("error: {message}\n"));
}

#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn an_undeclared_scope_is_unknown_scope_for_check_and_export() {
    let (_directory, repo) = init();
    for line in [
        &["check", "--scope", "nosuch"][..],
        &["export", "--scope", "nosuch"][..],
    ] {
        let error = refusal(command(&repo, line));
        assert_eq!(error["error"]["kind"], "unknown_scope", "{line:?}: {error}");
        let message = error["error"]["message"].as_str().unwrap();
        assert!(message.contains("nosuch"), "{line:?}: {error}");
    }
}

/// `check` gives a state file that does not parse as the typed failure that
/// `export` gives, with the hint in its own field.
#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn check_gives_a_damaged_state_file_as_invalid_state_file() {
    let (directory, repo) = init();
    let manifest = directory.path().join(".provenance/state/manifest.json");
    std::fs::write(&manifest, "not json\n").unwrap();

    let export = refusal(command(&repo, &["export"]));
    assert_eq!(export["error"]["kind"], "invalid_state_file", "{export}");
    assert!(export["error"]["hint"].is_string(), "{export}");

    let output = command(&repo, &["check", "--format", "json"])
        .output()
        .unwrap();
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1), "{text}");
    let check: Value =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"));
    assert_eq!(check, export);
}

/// The message of an `operation_failed` object gives each cause one time,
/// also when the text before it already holds the cause.
#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn an_operation_failed_message_gives_each_cause_one_time() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing");

    let error = refusal(command(missing.to_str().unwrap(), &["export"]));
    assert_eq!(error["error"]["kind"], "operation_failed", "{error}");
    let message = error["error"]["message"].as_str().unwrap();
    assert_eq!(message.matches("(os error").count(), 1, "{message}");
}
