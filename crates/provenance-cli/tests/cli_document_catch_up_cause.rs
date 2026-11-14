//! A document read whose catch-up fails on a state line that does not parse
//! gives the file, the line and the parse message of that line, in JSON and
//! in text.

use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

fn binary() -> &'static Path {
    assert_cmd::cargo::cargo_bin!("provenance")
}

/// A repository with one Requirement whose document was read one time, so
/// that the projection holds a revision.
fn read_once(directory: &Path) {
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
    let first = document(directory, &["--format", "json"]);
    assert!(first.status.success(), "{}", first.stderr);
}

/// Puts a string where the Requirement line holds a list of parents.
fn give_refines_a_string(directory: &Path) {
    let shard = directory.join(".provenance/state/scopes/default/requirements/req.jsonl");
    let text = std::fs::read_to_string(&shard).unwrap();
    let mut line: Value = serde_json::from_str(&text).unwrap();
    line["refines"] = json!("req_parent");
    std::fs::write(&shard, format!("{line}\n")).unwrap();
}

struct Output {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn document(directory: &Path, format: &[&str]) -> Output {
    let output = assert_cmd::Command::new(binary())
        .args(["api", "/requirements/req_alpha/document"])
        .args(format)
        .arg("--repo")
        .arg(directory)
        .output()
        .unwrap();
    Output {
        status: output.status,
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

/// The refusal that the error line of standard error gives.
fn refusal(output: &Output) -> Value {
    let line = output.stderr.trim().strip_prefix("Error: ").unwrap();
    serde_json::from_str(line).unwrap()
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_document_read_gives_the_line_that_stops_its_catch_up() {
    let directory = tempfile::tempdir().unwrap();
    read_once(directory.path());
    give_refines_a_string(directory.path());
    let expected = json!({
        "error": {
            "kind": "document_catch_up_failed",
            "cause": {
                "kind": "invalid_state_file",
                "file": "scopes/default/requirements/req.jsonl",
                "line": 1,
                "message": "invalid type: string \"req_parent\", expected a sequence"
            }
        },
        "meta": {}
    });

    for format in [&["--format", "json"][..], &[]] {
        let refused = document(directory.path(), format);

        assert_eq!(refused.status.code(), Some(1), "{format:?}");
        assert_eq!(refused.stdout, "", "{format:?}");
        assert_eq!(refusal(&refused), expected, "{format:?}");
    }
}
