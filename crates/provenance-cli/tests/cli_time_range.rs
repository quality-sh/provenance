//! A time field or a Discussion version past the range of the read
//! database, ±(2^53 − 1), in a committed record fails as a malformed field
//! fails: `check` refuses with the file and the line of the record, and a
//! read or a write over the record refuses as over a malformed record.
//!
//! No Rule: an implementation aid. It compares the refusal with the
//! refusal of a malformed field, so it holds the place of the range check
//! in the parse and not a text of its own.

use serde_json::Value;
use std::path::{Path, PathBuf};

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, provenance};

/// The one Resolution file of the repository.
fn resolution_file(root: &Path) -> PathBuf {
    let directory = root.join(".provenance/state/scopes/default/resolutions");
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    files.remove(0)
}

/// A repository with one Resolution whose `approved_at` holds `value`.
fn repository(value: Value) -> (tempfile::TempDir, String) {
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
    let path = resolution_file(directory.path());
    set_first_line(&path, "approved_at", value);
    (directory, repo)
}

/// Sets `field` of the first record of the file at `path` to `value`.
fn set_first_line(path: &Path, field: &str, value: Value) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut record: Value = serde_json::from_str(&lines[0]).unwrap();
    record[field] = value;
    lines[0] = record.to_string();
    std::fs::write(path, lines.join("\n") + "\n").unwrap();
}

/// A repository with one Discussion whose `version` holds `value`, and the
/// id of the Discussion.
fn discussion(value: Value) -> (tempfile::TempDir, String, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let started = support::review_paths::json_output(provenance().args([
        "req_a", "discuss", "--repo", &repo, "--body", "hello", "--format", "json",
    ]));
    let id = started["receipt"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = directory
        .path()
        .join(".provenance/state/scopes/default/threads/discussions.jsonl");
    set_first_line(&path, "version", value);
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

/// One repository of a test and the id of its record.
struct Case<'a> {
    repo: &'a str,
    id: &'a str,
}

/// The exit code, and the output and error of `args` over `case` with the
/// parse messages left out and the id of the record as `<id>`. Each `<id>`
/// in `args` is the id of the record.
fn masked(case: &Case, args: &[&str]) -> (Option<i32>, String, String, String) {
    let args: Vec<&str> = args
        .iter()
        .map(|arg| if *arg == "<id>" { case.id } else { arg })
        .collect();
    let (code, out, err) = run(case.repo, &args);
    let mask = |text: &[u8]| without_messages(text).replace(case.id, "<id>");
    (
        code,
        mask(&out),
        mask(&err),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

/// What `args` over the record past the range give, beside the output of
/// the malformed record.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// `check` names the file and the value.
    Check,
    /// `invalid_state_file` with the file and the value.
    Located,
    /// The kind `kind`, as over the malformed record.
    Kind(&'static str),
}

/// `args` over `past` and over `malformed` give the same exit code and
/// output, with the parse messages left out. Over `past`, the code is 1 and
/// standard error holds `refusal`.
fn fails_as_malformed(past: &Case, malformed: &Case, args: &[&str], refusal: Refusal, file: &str) {
    let (past_code, past_out, past_err, past_text) = masked(past, args);
    let (code, out, err, _) = masked(malformed, args);
    assert_eq!(past_code, Some(1), "{args:?}: {past_text}");
    let names_the_line = past_text.contains(file) && past_text.contains("9007199254740992");
    let holds = match refusal {
        Refusal::Check => names_the_line && !past_text.contains("invalid_state_file"),
        Refusal::Located => names_the_line && past_text.contains("invalid_state_file"),
        Refusal::Kind(kind) => past_text.contains(&format!("\"kind\":\"{kind}\"")),
    };
    assert!(holds, "{args:?}: {past_text}");
    assert_eq!(
        (past_code, past_out, past_err),
        (code, out, err),
        "{args:?}"
    );
}

#[test]
fn a_committed_time_past_the_range_fails_as_a_malformed_field_fails() {
    let (_past_dir, past) = repository(Value::from(1_i64 << 53));
    let (_malformed_dir, malformed) = repository(Value::from("soon"));
    let past = Case {
        repo: &past,
        id: "resolution_a",
    };
    let malformed = Case {
        repo: &malformed,
        id: "resolution_a",
    };
    for args in [
        &["check", "--format", "json"][..],
        &["<id>", "get", "--format", "json"][..],
        &[
            "resolution_b",
            "create",
            "--type",
            "resolution",
            "--status",
            "approved",
            "--title",
            "Decision",
            "--requirement-id",
            "req_a",
            "--position",
            "Use the award threshold.",
            "--rationale",
            "The award clause sets the threshold.",
            "--format",
            "json",
        ][..],
    ] {
        let refusal = if args[0] == "check" {
            Refusal::Check
        } else {
            Refusal::Located
        };
        fails_as_malformed(&past, &malformed, args, refusal, "res.jsonl");
    }
}

#[test]
fn a_committed_discussion_version_past_the_range_fails_as_a_malformed_field_fails() {
    let (_past_dir, past, id) = discussion(Value::from(1_u64 << 53));
    let (_malformed_dir, malformed, malformed_id) = discussion(Value::from("soon"));
    let past = Case {
        repo: &past,
        id: &id,
    };
    let malformed = Case {
        repo: &malformed,
        id: &malformed_id,
    };
    for (args, refusal) in [
        (&["check", "--format", "json"][..], Refusal::Check),
        (
            &["<id>", "discussion", "--format", "json"][..],
            Refusal::Kind("resource_not_found"),
        ),
        (
            &[
                "<id>",
                "reply",
                "--expected-version",
                "1",
                "--body",
                "again",
                "--format",
                "json",
            ][..],
            Refusal::Located,
        ),
    ] {
        fails_as_malformed(&past, &malformed, args, refusal, "discussions.jsonl");
    }
}

/// A repository with one Requirement review whose `changed_at` holds
/// `value`, written by hand.
fn requirement_review(value: &Value) -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let record = serde_json::json!({"scope_id":"default","id":"requirement_review_a",
        "rule_id":"rule_a","requirement_id":"req_a","field":"statement","before":"Old.",
        "after":"New.","changed_at":value});
    std::fs::write(
        directory
            .path()
            .join(".provenance/state/scopes/default/requirements/review.jsonl"),
        record.to_string() + "\n",
    )
    .unwrap();
    (directory, repo)
}

#[test]
fn a_committed_review_time_past_the_range_fails_check_as_a_malformed_field_fails() {
    let (_past_dir, past) = requirement_review(&Value::from(1_i64 << 53));
    let (_malformed_dir, malformed) = requirement_review(&Value::from("soon"));
    fails_as_malformed(
        &Case {
            repo: &past,
            id: "requirement_review_a",
        },
        &Case {
            repo: &malformed,
            id: "requirement_review_a",
        },
        &["check", "--format", "json"],
        Refusal::Check,
        "review.jsonl",
    );
}
