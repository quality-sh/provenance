//! A document read whose catch-up fails on a state line that does not parse
//! gives the file, the line and the parse message of that line, and the steps
//! that correct the state, in JSON and in text, for a line of the wrong type
//! and for a line that is not JSON. A cause with a kind of its own and no file
//! gives its text as the message, with no hint.

use crate::state_lines::give_refines_a_string;
use fs2::FileExt;
use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;

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

struct Output {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn document(directory: &Path, format: &[&str]) -> Output {
    api(directory, "/requirements/req_alpha/document", format)
}

/// The output of `provenance api` for `path`.
fn api(directory: &Path, path: &str, format: &[&str]) -> Output {
    let output = assert_cmd::Command::new(binary())
        .args(["api", path])
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

/// The refusal on standard error: the one object with `--format json`, and the
/// text after `Error: ` without it.
fn refusal(output: &Output, format: &[&str]) -> Value {
    let line = output.stderr.trim();
    let line = if format.is_empty() {
        line.strip_prefix("Error: ").unwrap()
    } else {
        line
    };
    serde_json::from_str(line).unwrap()
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
#[verifies("rule_document_catch_up_refusal_gives_typed_cause", examples)]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn a_document_read_gives_the_line_that_stops_its_catch_up() {
    let directory = tempfile::tempdir().unwrap();
    read_once(directory.path());
    give_refines_a_string(directory.path(), "req_alpha");
    let expected = json!({
        "error": {
            "kind": "document_catch_up_failed",
            "file": "scopes/default/requirements/req.jsonl",
            "line": 1,
            "message": "invalid type: string \"req_parent\", expected a sequence",
            "hint": STATE_CORRECTIONS
        },
        "meta": {}
    });

    for format in [&["--format", "json"][..], &[]] {
        let refused = document(directory.path(), format);

        assert_eq!(refused.status.code(), Some(1), "{format:?}");
        assert_eq!(refused.stdout, "", "{format:?}");
        assert_eq!(refusal(&refused, format), expected, "{format:?}");
    }
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_document_read_gives_the_line_that_is_not_json() {
    let directory = tempfile::tempdir().unwrap();
    add_a_rule_line_that_is_not_json(directory.path());

    let refused = document(directory.path(), &["--format", "json"]);

    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(refused.stdout, "");
    assert_eq!(
        refusal(&refused, &["--format", "json"]),
        json!({
            "error": {
                "kind": "document_catch_up_failed",
                "file": "scopes/default/rules/rule.jsonl",
                "line": 2,
                "message": "expected value",
                "hint": STATE_CORRECTIONS
            },
            "meta": {}
        })
    );
}

/// A repository with a Requirement and a Rule whose projection holds a
/// revision, and a second Rule line that is not JSON.
fn add_a_rule_line_that_is_not_json(directory: &Path) {
    read_once(directory);
    assert_cmd::Command::new(binary())
        .args(["rule_alpha", "create", "--type", "rule", "--requirement-id"])
        .args(["req_alpha", "--statement", "The host starts in one second."])
        .arg("--repo")
        .arg(directory)
        .assert()
        .success();
    let shard = directory.join(".provenance/state/scopes/default/rules/rule.jsonl");
    let text = std::fs::read_to_string(&shard).unwrap();
    std::fs::write(&shard, format!("{text}invalid JSON\n")).unwrap();
}

/// A record read and a list read answer at the stored serial and give the
/// file and the line of the Rule line that is not JSON in `freshness_error`.
#[test]
#[verifies("rule_failed_freshness_answers_at_stored_serial", examples)]
fn a_record_read_and_a_list_read_give_the_line_that_is_not_json() {
    let directory = tempfile::tempdir().unwrap();
    add_a_rule_line_that_is_not_json(directory.path());
    let line = "scopes/default/rules/rule.jsonl line 2: expected value";

    for path in ["/requirements/req_alpha", "/requirements"] {
        let read = api(directory.path(), path, &["--format", "json"]);

        assert!(read.status.success(), "{path}: {}", read.stderr);
        let answer: Value = serde_json::from_str(&read.stdout).unwrap();
        assert_eq!(
            answer["meta"]["stamp"]["policy"], "catch_up_failed",
            "{path}"
        );
        let text = answer["meta"]["freshness_error"].as_str().unwrap();
        assert!(text.contains(line), "{path}: {text}");
    }
}

/// Holds the close lock of the read database of the repository at
/// `directory`, as another close does.
fn hold_the_close_lock(directory: &Path) -> std::fs::File {
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(".provenance/cache/provenance.db.close.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    lock
}

/// A cache close that does not finish in time has a kind of its own and no
/// file. As the cause of a failed catch-up, it gives its text as the
/// message.
#[test]
#[verifies("rule_document_catch_up_refusal_gives_typed_cause", examples)]
fn a_document_read_gives_the_text_of_a_cause_with_a_kind_of_its_own() {
    let directory = tempfile::tempdir().unwrap();
    read_once(directory.path());
    give_refines_a_string(directory.path(), "req_alpha");
    let lock = hold_the_close_lock(directory.path());
    // Each close waits for the lock for at most 10 seconds, a fixed value
    // that no public input makes shorter. The close after the failed
    // catch-up passes that deadline. The read at the stored serial then
    // closes under the same lock, so a lock held until the answer makes
    // that close pass its deadline too. The lock is free after 16 seconds:
    // the first close must start in the first 6 seconds, and the second
    // close, which starts after 10 seconds, takes the lock in time.
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(16));
        FileExt::unlock(&lock).unwrap();
    });

    let refused = document(directory.path(), &["--format", "json"]);

    release.join().unwrap();
    assert_eq!(refused.status.code(), Some(1), "{}", refused.stderr);
    assert_eq!(refused.stdout, "");
    assert_eq!(
        refusal(&refused, &["--format", "json"]),
        json!({
            "error": {
                "kind": "document_catch_up_failed",
                "message": "the cache close did not finish in time; retry the read"
            },
            "meta": {}
        })
    );
}
