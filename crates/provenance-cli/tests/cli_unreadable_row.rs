//! A stored row that the store cannot read is state of the repository. A
//! read or a write of its family refuses with `invalid_state_file`, and the
//! refusal gives the file and the line of the row.
//!
//! A row that the store cannot read back as one record does not parse as a
//! record, so `rule_state_parse_refusal_names_the_line` states this refusal.

use provenance_macros::verifies;
use serde_json::{json, Value};

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, json_output, provenance};

const REQUIREMENTS: &str = ".provenance/state/scopes/default/requirements/req.jsonl";
const RETIRED: &str = "record req_a contains legacy field `retired`; canonical JSONL requires \
    record-deletion migration 026 before this build can read it";

/// Replaces the first requirement row with the output of `change`.
fn change_requirement_row(directory: &tempfile::TempDir, change: impl FnOnce(&str) -> String) {
    let path = directory.path().join(REQUIREMENTS);
    let text = std::fs::read_to_string(&path).unwrap();
    let row = change(text.lines().next().unwrap());
    std::fs::write(&path, row + "\n").unwrap();
}

/// A repository with one requirement row, changed by `change`.
fn repository(change: impl FnOnce(&str) -> String) -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    change_requirement_row(&directory, change);
    (directory, repo)
}

/// The refusal object on the first line of standard error, after the exit
/// code 1 and an empty standard output are asserted.
fn refusal(output: &std::process::Output) -> Value {
    let text = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(output.stdout.is_empty(), "{text}");
    let line = text.lines().next().unwrap_or_default();
    let object = line.strip_prefix("Error: ").unwrap_or(line);
    serde_json::from_str(object).unwrap_or_else(|error| panic!("{error}: {text}"))
}

fn state_file_refusal(message: &str) -> Value {
    json!({
        "error": {
            "kind": "invalid_state_file",
            "file": "scopes/default/requirements/req.jsonl",
            "line": 1,
            "message": message,
        },
        "meta": {},
    })
}

fn update(repo: &str) -> std::process::Output {
    let read = json_output(provenance().args(["req_a", "get", "--repo", repo, "--format", "json"]));
    let etag = read["edit"]["etag"].as_str().unwrap().to_owned();
    provenance()
        .args([
            "req_a",
            "update",
            "--repo",
            repo,
            "--if-match",
            &etag,
            "--statement",
            "The system records two topics.",
            "--format",
            "json",
        ])
        .output()
        .unwrap()
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_typed_change_of_a_row_with_a_repeated_member_refuses_at_the_row() {
    let (_directory, repo) = repository(|row| {
        format!(
            "{},\"extension\":1,\"extension\":2}}",
            row.strip_suffix('}').unwrap()
        )
    });
    assert_eq!(
        refusal(&update(&repo)),
        state_file_refusal(
            "repeated top-level member `extension` cannot survive a typed record change"
        )
    );
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn each_read_and_write_of_a_row_with_the_retired_field_refuses_at_the_row() {
    let (_directory, repo) =
        repository(|row| format!("{},\"retired\":true}}", row.strip_suffix('}').unwrap()));
    let create = provenance()
        .args([
            "req_b",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The system records two topics.",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(refusal(&create), state_file_refusal(RETIRED));
    let read = provenance()
        .args(["api", "/requirements/req_a", "--repo", &repo])
        .output()
        .unwrap();
    assert_eq!(refusal(&read), state_file_refusal(RETIRED));
}
