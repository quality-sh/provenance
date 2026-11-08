//! A store failure of no known kind keeps the code `write_failed` or
//! `read_failed`, and the refusal object gives the cause chain in `cause`.
//! A path below the state directory in the cause is relative to that
//! directory.
//!
//! Implementation aid: no Rule covers the cause of a failure of no known kind
//! yet, so these tests have no Rule binding.

use serde_json::Value;

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, json_output, provenance};

const REQUIREMENTS: &str = ".provenance/state/scopes/default/requirements/req.jsonl";

/// Replaces the first requirement row with the output of `change`.
fn change_requirement_row(directory: &tempfile::TempDir, change: impl FnOnce(&str) -> String) {
    let path = directory.path().join(REQUIREMENTS);
    let text = std::fs::read_to_string(&path).unwrap();
    let row = change(text.lines().next().unwrap());
    std::fs::write(&path, row + "\n").unwrap();
}

/// A repository whose requirement row repeats an unknown top-level member, so
/// that a typed change of the row fails with no known kind.
fn repeated_member() -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    change_requirement_row(&directory, |row| {
        format!(
            "{},\"extension\":1,\"extension\":2}}",
            row.strip_suffix('}').unwrap()
        )
    });
    (directory, repo)
}

/// A repository whose requirement row holds the removed `retired` field, so
/// that each read and write of the family fails with no known kind.
fn legacy_field() -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    change_requirement_row(&directory, |row| {
        format!("{},\"retired\":true}}", row.strip_suffix('}').unwrap())
    });
    (directory, repo)
}

fn update(repo: &str, json: bool) -> std::process::Output {
    let read = json_output(provenance().args(["req_a", "get", "--repo", repo, "--format", "json"]));
    let etag = read["edit"]["etag"].as_str().unwrap().to_owned();
    let mut command = provenance();
    command.args([
        "req_a",
        "update",
        "--repo",
        repo,
        "--if-match",
        &etag,
        "--statement",
        "The system records two topics.",
    ]);
    if json {
        command.args(["--format", "json"]);
    }
    command.output().unwrap()
}

/// The refusal object on standard error of a command that exits 1, with or
/// without the `Error: ` prefix.
fn refusal(output: &std::process::Output) -> Value {
    let text = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{text}");
    let line = text.lines().next().unwrap_or_default();
    let object = line.strip_prefix("Error: ").unwrap_or(line);
    serde_json::from_str(object).unwrap_or_else(|error| panic!("{error}: {text}"))
}

/// Asserts the code, that the cause holds `expected`, and that no absolute
/// state path is in the refusal.
fn assert_cause(refusal: &Value, kind: &str, expected: &str) {
    assert_eq!(refusal["error"]["kind"], kind, "{refusal}");
    let cause = refusal["error"]["cause"].as_str().unwrap_or_default();
    assert!(cause.contains(expected), "{refusal}");
    assert!(!refusal.to_string().contains(".provenance"), "{refusal}");
}

#[test]
fn text_mode_prints_the_cause_of_a_write_failure_of_no_known_kind() {
    let (_directory, repo) = repeated_member();
    let output = update(&repo, false);
    let text = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(text.starts_with("Error: "), "{text}");
    assert_cause(
        &refusal(&output),
        "write_failed",
        "scopes/default/requirements/req.jsonl line 1: repeated top-level member `extension`",
    );
}

#[test]
fn json_mode_gives_the_cause_of_a_write_failure_of_no_known_kind() {
    let (_directory, repo) = repeated_member();
    let refusal = refusal(&update(&repo, true));
    assert_cause(
        &refusal,
        "write_failed",
        "scopes/default/requirements/req.jsonl line 1: repeated top-level member `extension`",
    );
    assert_eq!(refusal["meta"], serde_json::json!({}), "{refusal}");
}

#[test]
fn a_create_gives_the_cause_of_a_malformed_state_file() {
    let (_directory, repo) = legacy_field();
    let output = provenance()
        .args([
            "req_b",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The system keeps review feedback available.",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_cause(
        &refusal(&output),
        "write_failed",
        "scopes/default/requirements/req.jsonl line 1: record req_a contains legacy field `retired`",
    );
}

#[test]
fn an_api_read_gives_the_cause_of_a_malformed_state_file() {
    let (_directory, repo) = legacy_field();
    let output = provenance()
        .args(["api", "/requirements/req_a", "--repo", &repo])
        .output()
        .unwrap();
    assert_cause(
        &refusal(&output),
        "read_failed",
        "scopes/default/requirements/req.jsonl line 1: record req_a contains legacy field `retired`",
    );
}
