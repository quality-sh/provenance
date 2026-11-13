//! An integer past the range of the read database, ±(2^53 − 1), in a
//! committed record fails as a malformed field fails: `check` and a read
//! refuse with the file and the line of the record, and a read gives
//! `invalid_state_file`.
//!
//! No Rule: an implementation aid. The range is a limit of the read
//! database port, and no Rule of the graph states the parse refusal.

use serde_json::Value;
use std::path::{Path, PathBuf};

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, json_output, provenance};

/// The one file of `family` in the default scope whose name ends with
/// `suffix`.
fn state_file(root: &Path, family: &str, suffix: &str) -> PathBuf {
    let directory = root.join(".provenance/state/scopes/default").join(family);
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(suffix))
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    files.remove(0)
}

/// Sets `field` of the first line of `path` to `value`.
fn edit(path: &Path, field: &str, value: Value) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut record: Value = serde_json::from_str(&lines[0]).unwrap();
    record[field] = value;
    lines[0] = record.to_string();
    std::fs::write(path, lines.join("\n") + "\n").unwrap();
}

/// A repository with one Resolution whose `approved_at` holds `value`.
fn resolution(value: Value) -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    provenance()
        .args([
            "resolution_a",
            "create",
            "--type",
            "resolution",
            "--status",
            "approved",
            "--repo",
            &repo,
            "--title",
            "Decision",
            "--requirement-id",
            "req_a",
            "--position",
            "Use the award threshold.",
            "--rationale",
            "The award clause sets the threshold.",
        ])
        .assert()
        .success();
    edit(
        &state_file(directory.path(), "resolutions", ".jsonl"),
        "approved_at",
        value,
    );
    (directory, repo)
}

/// A repository with one Discussion whose `version` holds `value`, and
/// the id of the Discussion.
fn discussion(value: Value) -> (tempfile::TempDir, String, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let started = json_output(provenance().args([
        "req_a",
        "discuss",
        "--body",
        "Which threshold?",
        "--repo",
        &repo,
        "--format",
        "json",
    ]));
    let id = started["receipt"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned();
    edit(
        &state_file(directory.path(), "threads", "discussions.jsonl"),
        "version",
        value,
    );
    (directory, repo, id)
}

/// The text of `output` with the parse message of each line left out: the
/// value of `"message":"..."` in a JSON line, and the text after `line 1: `
/// in a text line.
fn without_messages(output: &[u8]) -> String {
    const MESSAGE: &str = "\"message\":\"";
    const LINE: &str = " line 1: ";
    String::from_utf8_lossy(output)
        .lines()
        .map(|line| match (line.find(MESSAGE), line.find(LINE)) {
            (Some(at), _) => {
                let start = at + MESSAGE.len();
                let rest = &line[start..];
                let mut escaped = false;
                let end = rest
                    .char_indices()
                    .find(|&(_, c)| {
                        let close = !escaped && c == '"';
                        escaped = !escaped && c == '\\';
                        close
                    })
                    .map_or(rest.len(), |(index, _)| index);
                format!("{}{}", &line[..start], &rest[end..])
            }
            (None, Some(at)) => line[..at + LINE.len()].to_owned(),
            (None, None) => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The exit code, standard output and standard error of `args` over `repo`.
fn run(repo: &str, args: &[&str]) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let output = provenance()
        .args(args)
        .args(["--repo", repo])
        .output()
        .unwrap();
    (output.status.code(), output.stdout, output.stderr)
}

/// Runs each of `commands` over `past` and `malformed`, and compares
/// the two after the parse messages: a value past the range fails as a
/// malformed value of the same field fails. `file` is the name of the
/// edited file, and `value` the value past the range.
fn fails_as_malformed(past: &str, malformed: &str, commands: &[&[&str]], file: &str, value: &str) {
    for (index, args) in commands.iter().enumerate() {
        let read = index > 0;
        let (past_code, past_out, past_err) = run(past, args);
        let (code, out, err) = run(malformed, args);
        let past_text = String::from_utf8_lossy(&past_err).into_owned();
        assert_eq!(past_code, Some(1), "{args:?}: {past_text}");
        assert!(past_text.contains(file), "{args:?}: {past_text}");
        assert!(past_text.contains(value), "{args:?}: {past_text}");
        assert_eq!(
            past_text.contains("invalid_state_file"),
            read,
            "{args:?}: {past_text}"
        );
        assert_eq!(
            (
                past_code,
                without_messages(&past_out),
                without_messages(&past_err)
            ),
            (code, without_messages(&out), without_messages(&err)),
            "{args:?}"
        );
    }
}

#[test]
fn a_committed_time_past_the_range_fails_as_a_malformed_field_fails() {
    let (_past_dir, past) = resolution(Value::from(1_i64 << 53));
    let (_malformed_dir, malformed) = resolution(Value::from("soon"));
    fails_as_malformed(
        &past,
        &malformed,
        &[
            &["check", "--format", "json"],
            &["resolution_a", "get", "--format", "json"],
        ],
        "res.jsonl",
        "9007199254740992",
    );
}

#[test]
fn a_discussion_version_past_the_range_fails_as_a_malformed_field_fails() {
    let (_past_dir, past, past_id) = discussion(Value::from(1_u64 << 53));
    let (_malformed_dir, malformed, _) = discussion(Value::from("two"));
    // The ids of the two Discussions differ, so the read of one by its id
    // is not compared with the other.
    let (past_code, _, past_err) = run(&past, &[&past_id, "discussion", "--format", "json"]);
    let past_text = String::from_utf8_lossy(&past_err).into_owned();
    assert_eq!(past_code, Some(1), "{past_text}");
    assert!(past_text.contains("invalid_state_file"), "{past_text}");
    assert!(past_text.contains("discussions.jsonl"), "{past_text}");
    assert!(past_text.contains("9007199254740992"), "{past_text}");
    fails_as_malformed(
        &past,
        &malformed,
        &[
            &["check", "--format", "json"],
            &["discussions", "--format", "json"],
        ],
        "discussions.jsonl",
        "9007199254740992",
    );
}
