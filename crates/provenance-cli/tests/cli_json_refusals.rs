//! With `--format json`, every refusal prints one `{error,meta}` object on
//! standard error after the `Error: ` prefix, with the recovery hint in
//! `error.hint`. Text mode keeps its wording.

use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use serde_json::Value;

const STALE: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
            "--disposition-actor-id",
            "reviewer",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            &repo,
            "--id",
            "req_a",
            "--statement",
            "The system records topics.",
        ])
        .assert()
        .success();
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
    let object = text
        .trim()
        .strip_prefix("Error: ")
        .unwrap_or_else(|| panic!("{args:?}: {text}"));
    let value: Value =
        serde_json::from_str(object).unwrap_or_else(|error| panic!("{args:?}: {error}: {text}"));
    assert_eq!(object.lines().count(), 1, "{text}");
    assert!(value["meta"].is_object(), "{text}");
    value
}

#[test]
#[verifies("rule_review_refusal_names_read_command", examples)]
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
    assert!(error["error"]["kind"].is_string());
    assert_eq!(error["error"]["message"], "invalid operation input");
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
