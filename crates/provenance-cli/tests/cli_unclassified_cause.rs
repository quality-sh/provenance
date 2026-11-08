//! A store failure of no known kind keeps its cause for the operator. Text
//! mode prints the cause after the refusal object. JSON mode prints only the
//! safe `write_failed` object, because the cause can hold absolute paths.
//!
//! Implementation aid: no Rule covers operator output for a failure of no
//! known kind yet, so these tests have no Rule binding.

use serde_json::Value;

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, json_output, provenance};

/// A repository whose requirement row repeats an unknown top-level member, so
/// that a typed change of the row fails with no known kind.
fn repeated_member() -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let path = directory
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let text = std::fs::read_to_string(&path).unwrap();
    let row = text.lines().next().unwrap();
    let row = format!(
        "{},\"extension\":1,\"extension\":2}}\n",
        row.strip_suffix('}').unwrap()
    );
    std::fs::write(&path, row).unwrap();
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

#[test]
fn text_mode_prints_the_cause_of_a_write_failure_of_no_known_kind() {
    let (_directory, repo) = repeated_member();
    let output = update(&repo, false);
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1), "{text}");
    assert!(
        text.starts_with(r#"Error: {"error":{"kind":"write_failed"},"meta":{}}"#),
        "{text}"
    );
    assert!(text.contains("Caused by:"), "{text}");
    assert!(
        text.contains("repeated top-level member `extension`"),
        "{text}"
    );
}

#[test]
fn json_mode_keeps_the_cause_out_of_the_write_failure() {
    let (_directory, repo) = repeated_member();
    let output = update(&repo, true);
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1), "{text}");
    let refusal: Value =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"));
    assert_eq!(
        refusal,
        serde_json::json!({"error": {"kind": "write_failed"}, "meta": {}})
    );
}
