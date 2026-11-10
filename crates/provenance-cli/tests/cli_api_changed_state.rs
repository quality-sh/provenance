//! An `api` list read after a hand edit makes a state row invalid. The
//! default policy answers at the stored serial and says so in the stamp and
//! in `freshness_error`. The `refuse_stale` policy refuses the read.

use provenance_macros::verifies;
use serde_json::Value;
use std::path::Path;

fn binary() -> &'static Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

/// A repository with one requirement, read one time so that the projection
/// holds a revision. Returns the first answer.
fn read_once(directory: &Path) -> Value {
    let repo = directory.to_str().unwrap();
    assert_cmd::Command::new(binary())
        .args(["init", "--path", repo, "--scope", "default"])
        .assert()
        .success();
    assert_cmd::Command::new(binary())
        .args(["req_alpha", "create", "--type", "requirement"])
        .args(["--statement", "The host starts.", "--repo", repo])
        .assert()
        .success();
    let first = list(directory);
    assert!(first.status.success(), "{}", first.stderr);
    assert_eq!(first.body["meta"]["stamp"]["policy"], "catch_up");
    first.body
}

struct Output {
    status: std::process::ExitStatus,
    body: Value,
    stderr: String,
}

fn list(directory: &Path) -> Output {
    let output = assert_cmd::Command::new(binary())
        .args(["api", "/requirements", "--format", "json", "--repo"])
        .arg(directory)
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let body = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    Output {
        status: output.status,
        body,
        stderr,
    }
}

/// Puts a status that no requirement can have into the row on disk.
fn corrupt_the_row(directory: &Path) {
    let shard = directory.join(".provenance/state/scopes/default/requirements/req.jsonl");
    let content = std::fs::read_to_string(&shard).unwrap();
    let edited = content.replace("\"status\":\"active\"", "\"status\":\"bogus\"");
    assert_ne!(edited, content);
    std::fs::write(&shard, edited).unwrap();
}

#[test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
fn a_list_read_after_an_invalid_edit_answers_at_the_stored_serial() {
    let directory = tempfile::tempdir().unwrap();
    let first = read_once(directory.path());
    corrupt_the_row(directory.path());

    let second = list(directory.path());

    assert!(second.status.success(), "{}", second.stderr);
    let stamp = &second.body["meta"]["stamp"];
    assert_eq!(stamp["policy"], "catch_up_failed");
    assert_eq!(stamp["serial"], first["meta"]["stamp"]["serial"]);
    assert_eq!(stamp["digest"], first["meta"]["stamp"]["digest"]);
    assert_eq!(second.body["data"], first["data"]);
    let error = second.body["meta"]["freshness_error"].as_str().unwrap();
    assert!(
        error.contains("req.jsonl line 1") && error.contains("bogus"),
        "{error}"
    );
}

#[test]
#[verifies("rule_refuse_stale_names_the_moved_units", examples)]
fn a_refuse_stale_list_read_after_an_invalid_edit_refuses() {
    let directory = tempfile::tempdir().unwrap();
    read_once(directory.path());
    corrupt_the_row(directory.path());
    std::fs::write(
        directory.path().join(".provenance/settings.json"),
        r#"{"read":{"freshness_policy":"refuse_stale"}}"#,
    )
    .unwrap();

    let refused = list(directory.path());

    assert!(!refused.status.success());
    assert_eq!(refused.body, Value::Null, "no rows on standard output");
    let error = refused.stderr.trim().trim_start_matches("Error: ");
    let error: Value = serde_json::from_str(error).unwrap();
    assert_eq!(error["error"]["kind"], "stale");
    assert_eq!(error["error"]["moved"][0]["unit"], "scope:default");
}
