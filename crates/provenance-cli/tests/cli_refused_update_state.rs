//! An update that names a record that does not exist refuses with
//! `missing_reference`, and each file of the state directory keeps its bytes.
//!
//! These tests have no Rule binding: no active Rule states that a refused
//! update changes no file. The tests of a stale `--if-match` together with a
//! missing target are characterization tests of the order of the two errors:
//! no active Rule states that order.

use crate::support::review_paths::{init, provenance};
use crate::support::state_files::state_files;
use serde_json::{json, Value};

const REFUSAL: &str = "Error: {\"error\":{\"kind\":\"missing_reference\"},\"meta\":{}}\n";

/// A repository with one Source, Requirement, Resolution and Rule, and its
/// path as the `--repo` value.
fn records() -> (tempfile::TempDir, String) {
    let (directory, repo) = init();
    for (id, kind, body) in [
        (
            "src_a",
            "source",
            r#"{"name":"Source","source_type":"policy"}"#,
        ),
        (
            "req_a",
            "requirement",
            r#"{"statement":"The system keeps the records."}"#,
        ),
        (
            "res_a",
            "resolution",
            r#"{"title":"Decision","position":"Keep the records.",
                "rationale":"The records are necessary.","requirement_ids":["req_a"]}"#,
        ),
        (
            "rule_a",
            "rule",
            r#"{"statement":"The system keeps the records.","requirement_ids":["req_a"]}"#,
        ),
    ] {
        provenance()
            .args([id, "create", "--type", kind, "--stdin", "--repo", &repo])
            .write_stdin(body)
            .assert()
            .success();
    }
    (directory, repo)
}

/// Runs `<id> update <flag> <target>` and checks the refusal and the files.
fn assert_refused_with_no_change(id: &str, flag: &str, target: &str) {
    let (directory, repo) = records();
    let before = state_files(directory.path());

    let output = provenance()
        .args([id, "update", flag, target, "--repo", &repo])
        .output()
        .unwrap();

    assert_eq!(
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ),
        (Some(1), String::new(), REFUSAL.to_owned()),
        "{id} update {flag} {target}"
    );
    assert_eq!(
        state_files(directory.path()),
        before,
        "{id} update {flag} {target} changed a state file"
    );
}

#[test]
fn a_resolution_update_that_names_no_requirement_changes_no_file() {
    assert_refused_with_no_change("res_a", "--requirement-ids", "req_missing");
}

#[test]
fn a_resolution_update_that_supersedes_no_resolution_changes_no_file() {
    assert_refused_with_no_change("res_a", "--supersedes", "res_missing");
}

#[test]
fn a_rule_update_that_names_no_requirement_changes_no_file() {
    assert_refused_with_no_change("rule_a", "--requirement-ids", "req_missing");
}

#[test]
fn a_rule_update_that_names_no_resolution_changes_no_file() {
    assert_refused_with_no_change("rule_a", "--resolution-ids", "res_missing");
}

#[test]
fn a_source_update_that_supersedes_no_source_changes_no_file() {
    assert_refused_with_no_change("src_a", "--supersedes", "src_missing");
}

/// The JSON of `<id> get`.
fn get(repo: &str, id: &str) -> Value {
    let output = provenance()
        .args([id, "get", "--format", "json", "--repo", repo])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{id} get");
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Reads the edit `etag` of `id`, changes the text field `change` of the record, and
/// then runs `<id> update <flag> <target> --if-match <etag of the read>`.
/// Checks that the stale guard wins over the missing target and that no state
/// file changes.
fn assert_stale_guard_wins(id: &str, record_kind: &str, change: &str, flag: &str, target: &str) {
    let (directory, repo) = records();
    let stale = get(&repo, id)["edit"]["etag"].as_str().unwrap().to_owned();
    provenance()
        .args([id, "update", change, "Changed.", "--repo", &repo])
        .assert()
        .success();
    let current = get(&repo, id)["edit"]["etag"].as_str().unwrap().to_owned();
    assert_ne!(current, stale);
    let before = state_files(directory.path());

    let output = provenance()
        .args([
            id,
            "update",
            flag,
            target,
            "--if-match",
            &stale,
            "--format",
            "json",
            "--repo",
            &repo,
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let refusal = serde_json::from_str::<Value>(&stderr).ok();
    let hint = format!("run `provenance {id} --view review --scope default --repo {repo}`");
    assert_eq!(
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            refusal,
        ),
        (
            Some(1),
            String::new(),
            Some(json!({"error":{"current_etag":current,"hint":hint,
                "kind":"record_edit_conflict","record_kind":record_kind},"meta":{}})),
        ),
        "{id} update {flag} {target} --if-match <stale>: {stderr}"
    );
    assert_eq!(
        state_files(directory.path()),
        before,
        "{id} update {flag} {target} --if-match <stale> changed a state file"
    );
}

#[test]
fn a_stale_guard_wins_over_a_missing_requirement_of_a_resolution_update() {
    assert_stale_guard_wins(
        "res_a",
        "resolution",
        "--title",
        "--requirement-ids",
        "req_missing",
    );
}

#[test]
fn a_stale_guard_wins_over_a_missing_resolution_of_a_rule_update() {
    assert_stale_guard_wins(
        "rule_a",
        "rule",
        "--description",
        "--resolution-ids",
        "res_missing",
    );
}

#[test]
fn a_stale_guard_wins_over_a_missing_source_of_a_source_update() {
    assert_stale_guard_wins("src_a", "source", "--name", "--supersedes", "src_missing");
}
