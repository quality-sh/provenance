//! An update that names a record that does not exist refuses with
//! `missing_reference`, and each file of the state directory keeps its bytes.
//!
//! These tests have no Rule binding: no active Rule states that a refused
//! update changes no file.

use crate::support;
use std::collections::BTreeMap;
use std::path::PathBuf;
use support::review_paths::{init, provenance};

const REFUSAL: &str = "Error: {\"error\":{\"kind\":\"missing_reference\"},\"meta\":{}}\n";

/// A repository with one Source, Requirement, Resolution and Rule.
fn records() -> tempfile::TempDir {
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
    directory
}

/// The text of each file of the state directory, by path. Equal text is equal
/// bytes, and a failure shows the changed lines.
fn state_text(repo: &std::path::Path) -> BTreeMap<PathBuf, String> {
    support::state_files::state_files(repo)
        .into_iter()
        .map(|(path, bytes)| (path, String::from_utf8(bytes).unwrap()))
        .collect()
}

/// Runs `<id> update <flag> <target>` and checks the refusal and the files.
fn assert_refused_with_no_change(id: &str, flag: &str, target: &str) {
    let directory = records();
    let before = state_text(directory.path());

    let output = provenance()
        .args([id, "update", flag, target, "--repo"])
        .arg(directory.path())
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
        state_text(directory.path()),
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

/// A missing target wins over a stale `--if-match`, as for a Requirement,
/// Topic or Question update.
#[test]
fn a_missing_target_wins_over_a_stale_guard() {
    let directory = records();
    let read = provenance()
        .args(["api", "resolutions/res_a", "--format", "json", "--repo"])
        .arg(directory.path())
        .output()
        .unwrap();
    let read: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    let stale = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    provenance()
        .args(["res_a", "update", "--title", "Another decision", "--repo"])
        .arg(directory.path())
        .assert()
        .success();
    let before = state_text(directory.path());

    let output = provenance()
        .args(["res_a", "update", "--requirement-ids", "req_missing"])
        .args(["--if-match", &stale, "--repo"])
        .arg(directory.path())
        .output()
        .unwrap();

    assert_eq!(
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ),
        (Some(1), REFUSAL.to_owned())
    );
    assert_eq!(state_text(directory.path()), before);
}
