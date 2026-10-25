//! With `--format json`, a refused record command prints one `{error,meta}`
//! object on standard error, with the recovery hint in `error.hint`. Errors
//! outside the operation, such as a missing graph, stay text. Text mode
//! keeps its wording.

use predicates::prelude::*;
use provenance_macros::verifies;
use serde_json::Value;

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, json_output, provenance};

const STALE: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

fn init() -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    (directory, repo)
}

/// The one JSON object a refused JSON-mode command prints.
fn refusal(repo: &str, args: &[&str]) -> Value {
    let output = provenance()
        .args(args)
        .args(["--repo", repo, "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{args:?}");
    assert!(output.stdout.is_empty(), "{args:?}");
    let text = String::from_utf8(output.stderr).unwrap();
    let value: Value =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{args:?}: {error}: {text}"));
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(value["meta"].is_object(), "{text}");
    value
}

#[test]
#[verifies("rule_review_refusal_names_read_command", examples)]
#[verifies("rule_cli_json_refusal_object", examples)]
fn a_stale_etag_refusal_is_one_object_with_the_hint_as_a_field() {
    let (_directory, repo) = init();

    for form in [
        vec!["req_a", "update"],
        vec!["requirements", "req_a", "update"],
    ] {
        let mut args = form.clone();
        args.extend([
            "--if-match",
            STALE,
            "--statement",
            "The system records two topics.",
        ]);
        let error = refusal(&repo, &args);
        assert_eq!(
            error["error"]["kind"], "requirement_edit_conflict",
            "{form:?}"
        );
        assert!(error["error"]["current_etag"].is_string());
        let hint = error["error"]["hint"].as_str().unwrap();
        assert_eq!(
            hint,
            format!("run `provenance req_a --view review --scope default --repo {repo}`"),
        );
    }
}

#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn read_refusals_are_one_object_each() {
    let (_directory, repo) = init();

    let error = refusal(&repo, &["req_a", "--scope", "nope"]);
    assert_eq!(error["error"]["kind"], "unknown_scope");
    assert_eq!(error["error"]["message"], "unknown scope");
    let error = refusal(&repo, &["requirements", "req_a", "get", "--scope", "nope"]);
    assert_eq!(error["error"]["kind"], "unknown_scope");
    let error = refusal(&repo, &["req_missing"]);
    assert_eq!(error["error"]["kind"], "resource_not_found");
    assert_eq!(error["error"]["message"], "record does not exist");
    let error = refusal(&repo, &["requirements", "req_missing", "get"]);
    assert_eq!(error["error"]["kind"], "resource_not_found");
    let error = refusal(&repo, &["REQ-1", "get"]);
    assert_eq!(error["error"]["kind"], "operation_failed");
    assert_eq!(error["error"]["message"], "invalid operation input");
}

#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn action_refusals_are_one_object_each() {
    let (_directory, repo) = init();

    let error = refusal(
        &repo,
        &[
            "req_missing",
            "update",
            "--statement",
            "The record is absent.",
        ],
    );
    assert_eq!(error["error"]["kind"], "resource_not_found");
    assert_eq!(error["error"]["message"], "record does not exist");

    let started = json_output(provenance().args([
        "req_a", "discuss", "--repo", &repo, "--body", "hello", "--format", "json",
    ]));
    let discussion = started["receipt"]["discussion_id"].as_str().unwrap();
    let error = refusal(
        &repo,
        &[
            discussion,
            "reply",
            "--body",
            "again",
            "--expected-version",
            "99",
        ],
    );
    assert_eq!(
        error["error"]["kind"], "discussion_version_conflict",
        "{error}"
    );
    assert!(error["error"]["message"].is_string(), "{error}");
}

#[test]
#[verifies("rule_review_refusal_names_read_command", examples)]
fn text_mode_keeps_the_failure_and_the_scoped_hint() {
    let (_directory, repo) = init();

    provenance()
        .args([
            "req_a",
            "update",
            "--repo",
            &repo,
            "--if-match",
            STALE,
            "--statement",
            "The system records two topics.",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(r#"Error: {"error":{"#))
        .stderr(predicate::str::contains(format!(
            "; run `provenance req_a --view review --scope default --repo {repo}`"
        )));
    provenance()
        .args(["req_a", "--repo", &repo, "--scope", "nope"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("Error: unknown scope"));
}

#[test]
#[verifies("rule_cli_json_refusal_object", examples)]
fn a_missing_graph_stays_a_text_error_in_json_mode() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();

    provenance()
        .args(["req_a", "--repo", repo, "--format", "json"])
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::starts_with("Error: "))
        .stderr(predicate::str::contains("not initialized"));
}
