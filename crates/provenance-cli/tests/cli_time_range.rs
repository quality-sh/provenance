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

/// The refusal on the first line of standard error, with or without the
/// `Error: ` prefix.
fn refusal(stderr: &[u8]) -> Value {
    let text = String::from_utf8_lossy(stderr);
    let line = text.lines().next().unwrap_or_default();
    let object = line.strip_prefix("Error: ").unwrap_or(line);
    serde_json::from_str(object).unwrap_or_else(|error| panic!("{error}: {text}"))
}

/// Runs `check` and each of `commands` over `past` and `malformed`, and
/// compares the two after the parse messages: a value past the range
/// fails as a malformed value of the same field fails. Over `past`,
/// `check` names `file`, line 1 and `value`, and each of `commands` gives
/// the `invalid_state_file` refusal with that file, line and value.
fn fails_as_malformed(past: &str, malformed: &str, commands: &[&[&str]], file: &str, value: &str) {
    let message = format!(
        "invalid value: integer `{value}`, expected an integer from \
         -9007199254740991 to 9007199254740991"
    );
    let check: &[&str] = &["check", "--format", "json"];
    for args in std::iter::once(check).chain(commands.iter().copied()) {
        let (past_code, past_out, past_err) = run(past, args);
        let (code, out, err) = run(malformed, args);
        let past_text = String::from_utf8_lossy(&past_err).into_owned();
        assert_eq!(past_code, Some(1), "{args:?}: {past_text}");
        if args[0] == "check" {
            assert!(
                past_text.contains(&format!("{file} line 1: {message}")),
                "{past_text}"
            );
        } else {
            let error = &refusal(&past_err)["error"];
            assert_eq!(
                [
                    &error["kind"],
                    &error["file"],
                    &error["line"],
                    &error["message"]
                ],
                [
                    &Value::from("invalid_state_file"),
                    &Value::from(file),
                    &Value::from(1),
                    &Value::from(message.as_str())
                ],
                "{args:?}: {past_text}"
            );
        }
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

/// A read and a write of the Resolution family over an `approved_at` past
/// the range.
#[test]
fn a_committed_time_past_the_range_fails_as_a_malformed_field_fails() {
    let (_past_dir, past) = resolution(Value::from(1_i64 << 53));
    let (_malformed_dir, malformed) = resolution(Value::from("soon"));
    fails_as_malformed(
        &past,
        &malformed,
        &[
            &["resolution_a", "get", "--format", "json"],
            &[
                "resolution_b",
                "create",
                "--type",
                "resolution",
                "--status",
                "approved",
                "--title",
                "Second decision",
                "--requirement-id",
                "req_a",
                "--position",
                "Use the award date.",
                "--rationale",
                "The award clause sets the date.",
                "--format",
                "json",
            ],
        ],
        "scopes/default/resolutions/res.jsonl",
        "9007199254740992",
    );
}

/// A read of the Discussion, a list of Discussions, and a reply, over a
/// Discussion `version` past the range.
#[test]
fn a_discussion_version_past_the_range_fails_as_a_malformed_field_fails() {
    let (_past_dir, past, past_id) = discussion(Value::from(1_u64 << 53));
    let (_malformed_dir, malformed, _) = discussion(Value::from("two"));
    let (code, _, stderr) = run(&past, &[&past_id, "discussion", "--format", "json"]);
    let error = &refusal(&stderr)["error"];
    assert_eq!(code, Some(1));
    assert_eq!(error["kind"], "invalid_state_file", "{error}");
    assert_eq!(
        error["file"], "scopes/default/threads/discussions.jsonl",
        "{error}"
    );
    // The ids of the two Discussions differ, so the commands that name a
    // Discussion are not compared with the malformed one.
    fails_as_malformed(
        &past,
        &malformed,
        &[
            &["discussions", "--format", "json"],
            &[
                "req_a",
                "discuss",
                "--body",
                "Which date?",
                "--format",
                "json",
            ],
        ],
        "scopes/default/threads/discussions.jsonl",
        "9007199254740992",
    );
}
